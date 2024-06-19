use crate::serializer::Serializer;
use crate::slab::{AgentKey, BridgeKey, RemoteKey};
use crate::SessionKey;

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum InternalAddress {
    /// A local address
    Local(AgentKey),
    /// A remotge address including the bridge and the remote key
    Remote {
        /// The local bridge connecting the local router to the remote router
        local_session_key: SessionKey,
        /// The remote key of the agent on the remote router
        remote_address: RemoteKey,
        /// The serializer used by the remote
        remote_serializer: Serializer,
    },
}

impl From<u64> for InternalAddress {
    fn from(value: u64) -> Self {
        Self::Local(value.into())
    }
}

/// An address to an agent
#[derive(Debug, PartialEq, Clone)]
pub struct Address(pub(crate) InternalAddress);

impl Address {
    pub(crate) fn inner(&self) -> &InternalAddress {
        &self.0
    }
}

impl From<InternalAddress> for Address {
    fn from(value: InternalAddress) -> Self {
        Self(value)
    }
}
