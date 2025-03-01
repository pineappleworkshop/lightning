//! The data types used in the application state.

use fleek_crypto::{EthAddress, NodeNetworkingPublicKey, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use ink_quill::TranscriptBuilderInput;
use multiaddr::Multiaddr;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

use super::ReputationMeasurements;

/// The Id of a Service
pub type ServiceId = u32;

/// Application epoch number
pub type Epoch = u64;

#[derive(Serialize, Deserialize, Hash, Debug, Clone)]
pub enum Tokens {
    USDC,
    FLK,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct NodeServed {
    pub served: CommodityServed,
    pub stables_revenue: HpUfixed<6>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct TotalServed {
    pub served: CommodityServed,
    pub reward_pool: HpUfixed<6>,
}

pub type ServiceRevenue = HpUfixed<6>;

/// This is commodity served by each of the commodity types
type CommodityServed = Vec<u128>;

/// This is commodities served by different services in Fleek Network.
/// C-like enums used here to future proof for state, if we add more commodity types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FromPrimitive)]
#[repr(u8)]
pub enum CommodityTypes {
    Bandwidth = 0,
    Compute = 1,
    Gpu = 2,
}

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct ReportedReputationMeasurements {
    // TODO: Use NodeIndex instead.
    pub reporting_node: NodePublicKey,
    pub measurements: ReputationMeasurements,
}

/// Metadata, state stored in the blockchain that applies to the current block
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Metadata {
    Epoch,
    SupplyYearStart,
    TotalSupply,
    ProtocolFundAddress,
    NextNodeIndex,
    GovernanceAddress,
}

/// The Value enum is a data type used to represent values in a key-value pair for a metadata table
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Value {
    Epoch(u64),
    String(String),
    HpUfixed(HpUfixed<18>),
    AccountPublicKey(EthAddress),
    NextNodeIndex(u32),
}

/// Adjustable parameters that are stored in the blockchain
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[repr(u8)]
pub enum ProtocolParams {
    /// The time in seconds that an epoch lasts for. Genesis 24 hours(86400)
    EpochTime = 0,
    /// The size of the committee
    CommitteeSize = 1,
    /// The min FLK a node has to stake to participate in the network
    MinimumNodeStake = 2,
    /// The time in epochs a node has to be staked to participate in the network
    EligibilityTime = 3,
    /// The time in epochs a node has to wait to withdraw after unstaking
    LockTime = 4,
    /// The percentage of the reward pool the protocol gets
    ProtocolShare = 5,
    /// The percentage of the reward pool goes to edge nodes
    NodeShare = 6,
    /// The percentage of the reward pool goes to edge nodes
    ServiceBuilderShare = 7,
    /// The maximum target inflation rate in a year
    MaxInflation = 8,
    /// The max multiplier on rewards for locking
    MaxBoost = 9,
    /// The max amount of time tokens can be locked
    MaxStakeLockTime = 10,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct NodeInfo {
    /// The owner of this node
    pub owner: EthAddress,
    /// The BLS public key of the node which is used for our BFT DAG consensus
    /// multi signatures.
    pub public_key: NodePublicKey,
    /// Public key that is used for fast communication signatures for this node.
    pub network_key: NodeNetworkingPublicKey,
    /// The epoch that this node has been staked since,
    pub staked_since: Epoch,
    /// The amount of stake by the node.
    pub stake: Staking,
    /// The nodes primary internet address
    pub domain: Multiaddr,
    /// A vec of all of this nodes Narwhal workers
    pub workers: Vec<Worker>,
    /// The nonce of the node. Added to each transaction before signed to prevent replays and
    /// enforce ordering
    pub nonce: u64,
}

/// Struct that stores the information about the stake of amount of a node.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct Staking {
    /// How much FLK that is currently staked
    pub staked: HpUfixed<18>,
    /// The epoch until all stakes are locked for boosting rewards
    pub stake_locked_until: u64,
    /// How much FLK is locked pending withdraw
    pub locked: HpUfixed<18>,
    /// The epoch the locked FLK is eligible to be withdrawn
    pub locked_until: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct Worker {
    /// The public key of the worker
    pub public_key: NodeNetworkingPublicKey,
    /// The workers internet address
    pub address: Multiaddr,
    /// The address to the workers mempool
    pub mempool: Multiaddr,
}

/// Placeholder
/// Information about the services
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash)]
pub struct Service {
    /// the owner address that deploys the service and also recieves reward share
    pub owner: EthAddress,
    // TODO: can there be multiple types of commodity per service
    /// the commodity that service is going to serve
    pub commodity_type: CommodityTypes,
    /// TODO: List of circuits to prove a node should be slashed
    pub slashing: (),
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct Committee {
    pub members: Vec<NodePublicKey>,
    pub ready_to_change: Vec<NodePublicKey>,
    pub epoch_end_timestamp: u64,
}

impl TranscriptBuilderInput for Service {
    const TYPE: &'static str = "service";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        self.commodity_type.to_transcript_builder_input()
        // todo: check if implementation needs to change when slashing is implemented
    }
}

impl TranscriptBuilderInput for Tokens {
    const TYPE: &'static str = "Tokens";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            Tokens::USDC => b"USDC".to_vec(),
            Tokens::FLK => b"FLK".to_vec(),
        }
    }
}

impl TranscriptBuilderInput for CommodityTypes {
    const TYPE: &'static str = "commodity_types";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        match self {
            CommodityTypes::Bandwidth => b"Bandwidth".to_vec(),
            CommodityTypes::Compute => b"Compute".to_vec(),
            CommodityTypes::Gpu => b"Gpu".to_vec(),
        }
    }
}
