use serde::{Deserialize, Serialize};

mod application;
mod bridge;
mod compression;
mod misbehavior;
mod pod;
mod reputation;
mod response;
mod state;
mod transaction;

pub use application::*;
pub use bridge::*;
pub use compression::*;
pub use misbehavior::*;
pub use pod::*;
pub use reputation::*;
pub use response::*;
pub use state::*;
pub use transaction::*;

/// The physical address of a node where it can be reached, the port numbers are
/// omitted since each node is responsible to open the standard port numbers for
/// different endpoints and it is unfeasible for us to try to keep a record of
/// this information.
///
/// For example one case to make about this decision is the fact that endpoints
/// are part of an implementation detail and we don't really want that level of
/// book keeping about which parts of a healthy system a node is running, due to
/// the fact that different versions of the software might expose different endpoints
/// a node might offer metrics endpoint publicly while another node might close
/// this port. So it is up to the implementation to pick these ports for different
/// reasons and a node runner that is running an actual node on the mainnet should
/// not modify these default port numbers. Just like how 80 is the port for HTTP,
/// and 443 is the port for SSL traffic, we should chose our numbers and stick
/// with them.
// TODO: Use this type again.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub enum InternetAddress {
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
}
