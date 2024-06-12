use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::serializer::Serializer;
use crate::slab::{AgentKey, BridgeKey, RemoteKey};

#[derive(Debug, PartialEq, Clone)]
pub enum Address {
    Local(AgentKey),
    Remote {
        local_bridge_key: BridgeKey,
        remote_address: RemoteKey,
        remote_serializer: Serializer,
    },
}

impl From<u64> for Address {
    fn from(value: u64) -> Self {
        Self::Local(value.into())
    }
}

//pub enum AddressRequest {
//    Local(Vec<u8>),
//}
//
//pub enum AddressResponse {
//    Local(Key),
//}
