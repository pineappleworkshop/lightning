use std::{ops::Add, time::Duration};

use arrayref::array_ref;
use num::integer::Roots;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rand_distr::{Distribution, Normal};

use super::LatencyProvider;

const PING_DATA: &[u8] = include_bytes!("../ping.bin");
const COUNT: usize = 222;

#[inline(always)]
fn read(i: usize, j: usize) -> PingStat {
    let index = 16 * (i * COUNT + j);
    let buffer = &PING_DATA[index..];
    PingStat {
        min: u32::from_le_bytes(*array_ref![buffer, 0, 4]),
        avg: u32::from_le_bytes(*array_ref![buffer, 4, 4]),
        max: u32::from_le_bytes(*array_ref![buffer, 8, 4]),
        stddev: u32::from_le_bytes(*array_ref![buffer, 12, 4]),
        count: 30,
    }
}

/// A type that can be used to sample data from a custom distribution.
pub trait RegionToRegionDistribution: From<PingStat> {
    /// Sample a value from this distribution. Should return a number in microseconds.
    fn next<R: Rng>(&mut self, rng: &mut R) -> u32;
}

/// A latency provider with pre-filled real world ping data.
pub struct PingDataLatencyProvider<T: RegionToRegionDistribution = ClampNormalDistribution> {
    rng: ChaCha8Rng,
    rng2: ChaCha8Rng,
    node_to_region: Vec<usize>,
    region_to_region: Vec<T>,
}

/// A normal distribution which clamps the ping data to the min and max.
pub struct ClampNormalDistribution {
    distr: Normal<f32>,
    min: f32,
    max: f32,
}

impl RegionToRegionDistribution for ClampNormalDistribution {
    #[inline(always)]
    fn next<R: Rng>(&mut self, rng: &mut R) -> u32 {
        let n = self.distr.sample(rng).clamp(self.min, self.max);
        (n * 1_000.0) as u32
    }
}

impl From<PingStat> for ClampNormalDistribution {
    fn from(value: PingStat) -> Self {
        let avg = ((value.avg as f64) / 1000.0) as f32;
        let stddev = ((value.stddev as f64) / 1000.0) as f32;
        let min = ((value.min as f64) / 1000.0) as f32;
        let max = ((value.max as f64) / 1000.0) as f32;
        Self {
            distr: Normal::new(avg, stddev).unwrap(),
            min,
            max,
        }
    }
}

impl<T: RegionToRegionDistribution> Default for PingDataLatencyProvider<T> {
    fn default() -> Self {
        Self {
            rng: ChaCha8Rng::from_seed([17; 32]),
            rng2: ChaCha8Rng::from_seed([11; 32]),
            node_to_region: Vec::new(),
            region_to_region: Vec::new(),
        }
    }
}

impl<T: RegionToRegionDistribution> LatencyProvider for PingDataLatencyProvider<T> {
    fn init(&mut self, number_of_nodes: usize) {
        let mut rng = ChaCha8Rng::from_seed([0; 32]);

        self.node_to_region = Vec::with_capacity(number_of_nodes);
        self.node_to_region
            .resize_with(number_of_nodes, || rng.gen::<usize>() % COUNT);

        self.region_to_region = Vec::with_capacity(COUNT * COUNT);
        for i in 0..COUNT {
            for j in 0..COUNT {
                self.region_to_region.push(read(i, j).into());
            }
        }
    }

    fn get(&mut self, a: usize, b: usize) -> Duration {
        if a == b {
            let micro = self.rng2.gen_range(14..25);
            return Duration::from_micros(micro / 2);
        }

        let region_a = self.node_to_region[a];
        let region_b = self.node_to_region[b];
        let index = region_a * COUNT + region_b;
        let entry = &mut self.region_to_region[index];
        let sample = entry.next(&mut self.rng);
        Duration::from_micros((sample / 2) as u64)
    }
}

/// The statistic data for a ping measurement between two servers.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PingStat {
    /// The minimum recorded ping.
    pub min: u32,
    /// The average recorded ping.
    pub avg: u32,
    /// The maximum recorded ping.
    pub max: u32,
    /// The standard deviation of the recording.
    pub stddev: u32,
    /// The number of runs.
    pub count: u32,
}

/// Merge two measurements into one measurement.
impl Add<PingStat> for PingStat {
    type Output = PingStat;

    fn add(self, rhs: PingStat) -> Self::Output {
        if self.count == 0 {
            return rhs;
        }

        if rhs.count == 0 {
            return self;
        }

        // The goal is to merge two standard deviations into one.
        //
        // Let's say data set one has standard deviation `s` and `N` data points.
        //
        // We observe:
        //
        // s        = √(∑(x_i - avg)^2 / N)
        // s^2      = ∑(x_i - avg)^2 / N
        // N * s^2  = ∑(x_i - avg)^2
        //          = ∑(x_i ^ 2 + avg^2 - 2 * x_i * avg)
        //          = N * avg^2 - 2avg∑x_i + ∑(x_i ^ 2)    since:
        //                                                   avg      = ∑x_i / N
        //                                                   ∑x_i     = avg * N
        //          = N * avg^2 - 2 * avg * avg * N + ∑(x_i ^ 2)
        //          = N * avg^2 - 2 * avg^2 * N + ∑(x_i ^ 2)
        //          = ∑(x_i ^ 2) - N * avg^2
        //          ^-- EQ-01
        //
        //  --> ∑(x_i ^ 2)  = N * s^2 + N * avg^2
        //                  = N * (s^2 + avg^2)
        //
        // The standard deviation of the merged data set one containing `N` element,
        // and the second one containing `M` elements can be computed as following.
        //
        // Notation:
        // `t`: the standard deviation of the second set.
        // `y_i`: the `i`-th member of the second set.
        // `o`: the stddev of the merged set.
        //
        // o^2              = (∑(x_i - merged_avg)^2 + ∑(y_i - merged_avg)^2) / (N + M)
        //
        // Via EQ-01 we can write:
        // (N + M) * o^2    = ∑(x_i ^ 2) +  ∑(y_i ^ 2) - (N + M) * merged_avg^2
        //      = N * (s^2 + avg_x^2) + M * (t^2 + avg_y^2) - (N + M) * merged_avg^2
        //      = N * (s^2 + avg_x^2) + M * (t^2 + avg_y^2) - (N + M) * merged_avg^2
        //
        // --> o
        //  = √( (N * (s^2 + avg_x^2) + M * (t^2 + avg_y^2)) / (N + M) -  merged_avg^2)
        //
        // and we know `merged_avg = (N * avg_x + M * avg_y) / (N + M)`.

        let n = self.count as u128;
        let avg_x = self.avg as u128;
        let s = self.stddev as u128;
        let m = rhs.count as u128;
        let avg_y = rhs.avg as u128;
        let t = rhs.stddev as u128;

        let merged_avg = (n * avg_x + m * avg_y) / (n + m);
        let o = n * (s * s + avg_x * avg_x) + m * (t * t + avg_y * avg_y);
        let o = o / (n + m);
        let o = o - merged_avg * merged_avg;
        let o = o.sqrt();

        let stddev = o as u32;
        let avg = merged_avg as u32;
        let min = self.min.min(rhs.min);
        let max = self.max.max(rhs.max);
        let count = self.count + rhs.count;

        Self {
            min,
            avg,
            max,
            stddev,
            count,
        }
    }
}

#[test]
fn test_data_count() {
    let count = (PING_DATA.len() / 16).sqrt();
    assert_eq!(COUNT, count);
}
