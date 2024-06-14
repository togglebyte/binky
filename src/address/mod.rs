use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::serializer::Serializer;
use crate::slab::{AgentKey, BridgeKey, RemoteKey};

/// An address to an agent
#[derive(Debug, PartialEq, Clone)]
pub enum Address {
    /// A local address
    Local(AgentKey),
    /// A remotge address including the bridge and the remote key
    Remote {
        /// The local bridge connecting the local router to the remote router
        local_bridge_key: BridgeKey,
        /// The remote key of the agent on the remote router
        remote_address: RemoteKey,
        /// The serializer used by the remote
        remote_serializer: Serializer,
    },
}

impl From<u64> for Address {
    fn from(value: u64) -> Self {
        Self::Local(value.into())
    }
}
