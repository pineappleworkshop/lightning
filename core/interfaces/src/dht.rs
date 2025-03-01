use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::blake3::derive_key;
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature, PublicKey};
use ink_quill::TranscriptBuilder;
use serde::{Deserialize, Serialize};

use crate::{SignerInterface, ToDigest, TopologyInterface, WithStartAndShutdown};

const FN_DHT_ENTRY_DOMAIN: &str = "FLEEK_NETWORK_DHT_ENTRY";

/// The distributed hash table for Fleek Network.
#[async_trait]
pub trait DhtInterface: WithStartAndShutdown + Sized {
    // -- DYNAMIC TYPES

    type Topology: TopologyInterface;

    // -- BOUNDED TYPES
    // empty

    async fn init<S: SignerInterface>(
        signer: &S,
        topology: Arc<Self::Topology>,
    ) -> anyhow::Result<Self>;

    /// Put a key-value pair into the distributed hash table.
    fn put(&self, prefix: KeyPrefix, key: &[u8], value: &[u8]);

    /// Return one value associated with the given key.
    async fn get(&self, prefix: KeyPrefix, key: &[u8]) -> Option<TableEntry>;
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[repr(u8)]
pub enum KeyPrefix {
    /// The content registry keys.
    ContentRegistry,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TableEntry {
    /// The name space of this key-value pair.
    pub prefix: KeyPrefix,
    /// The raw key.
    pub key: Vec<u8>,
    /// The raw value.
    pub value: Vec<u8>,
    /// The originator of this key-value relation.
    pub source: NodeNetworkingPublicKey,
    /// The signature from the source committing to this key-value.
    pub signature: Option<NodeNetworkingSignature>,
}

impl ToDigest for TableEntry {
    fn to_digest(&self) -> [u8; 32] {
        let tb = TranscriptBuilder::empty(FN_DHT_ENTRY_DOMAIN)
            .with("prefix", &(self.prefix as u8))
            .with("key", &self.key)
            .with("value", &self.value)
            .with("source", &self.source.0);
        derive_key(tb.get_domain(), &tb.compile())
    }
}

impl TableEntry {
    /// Returns true if a signature is present on this entry. This does not mean
    /// that the signature is actually valid, only that it does exists.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Returns true if the signature on this entry is valid. Returns `false` if
    /// either there is no signature present or if the signature is invalid.
    pub fn is_signature_valid(&self) -> bool {
        if let Some(sig) = &self.signature {
            let digest = self.to_digest();
            self.source.verify(sig, &digest)
        } else {
            false
        }
    }
}
