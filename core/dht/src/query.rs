use std::net::SocketAddr;

use fleek_crypto::NodeNetworkingPublicKey;
use serde::{Deserialize, Serialize};

use crate::table::TableKey;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub address: SocketAddr,
    pub key: NodeNetworkingPublicKey,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Query {
    Find { find_value: bool, target: TableKey },
    // Todo: This may not fit on a datagram
    // but we will delegate this task to an
    // encrypted channel.
    Store { key: TableKey, value: Vec<u8> },
    Ping,
}

#[repr(u8)]
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum MessageType {
    Query = 0x01 << 0,
    Response = 0x01 << 1,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub ty: MessageType,
    // Channel on which to route the response.
    pub id: u64,
    // Random value used that must be returned in response.
    pub token: u64,
    // Sender's public key.
    pub sender_key: NodeNetworkingPublicKey,
    // Payload of message.
    pub payload: Vec<u8>,
}

// Todo: Create some chunking strategy
// to avoid sending datagrams larger than 512.
#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    pub nodes: Vec<NodeInfo>,
    pub value: Option<Vec<u8>>,
}
