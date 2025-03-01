use std::io::{Read, Write};

use blake3_stream::*;
use blake3_tree::blake3::tree::{HashTree, HashTreeBuilder};
use criterion::*;

pub const SIZES: &[usize] = &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512];
pub const UNITS: &[(&str, usize)] = &[("KB", 1024), ("MB", 1024 * 1024)];

fn get_content_and_tree(len: usize) -> (Vec<u8>, HashTree) {
    let content = vec![0x80; len];
    let mut tree_builder = HashTreeBuilder::new();
    tree_builder.update(&content);

    (content, tree_builder.finalize())
}

fn bench(c: &mut Criterion) {
    let mut decode = c.benchmark_group("Verified Decode");

    for (name, unit) in UNITS {
        for &size in SIZES {
            let length = unit * size;
            decode.throughput(Throughput::Bytes(length as u64));
            let (content, tree) = get_content_and_tree(length);
            let mut encoded_buffer = Vec::new();
            let mut encoder = Encoder::new(&mut encoded_buffer, length, tree.clone()).unwrap();
            encoder.write_all(&content).unwrap();
            encoder.flush().unwrap();

            // benchmark encode
            decode.bench_with_input(BenchmarkId::new(name.to_string(), size), &length, |b, _| {
                b.iter(|| {
                    let mut decoder =
                        VerifiedDecoder::new(encoded_buffer.as_slice(), tree.hash.into());
                    let mut decoded_buffer = Vec::with_capacity(length);
                    decoder.read_to_end(&mut decoded_buffer).unwrap();
                })
            });
        }
    }

    decode.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
