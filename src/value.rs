use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::serializer::Serializer;
use crate::slab::{AgentKey, BridgeKey, RemoteKey};

pub(crate) type AnyValue = Box<dyn std::any::Any + Send>;

#[derive(Debug)]
pub(crate) struct RemoteVal<T>(pub(crate) T);

impl<T> Deref for RemoteVal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct Initial {
    payload: Box<[u8]>,
    sender: AgentKey,
    pub(crate) local_bridge_key: BridgeKey,
    recipient: RemoteKey,
}

impl RemoteVal<Initial> {
    pub(crate) fn new(
        payload: Box<[u8]>,
        sender: AgentKey,
        local_bridge_key: BridgeKey,
        recipient: RemoteKey,
    ) -> Self {
        Self(Initial {
            payload,
            sender,
            local_bridge_key,
            recipient,
        })
    }

    pub(crate) fn next(self, reply_serializer: Serializer) -> RemoteVal<Outgoing> {
        let initial = self.0;
        let sender = reply_serializer.serialize(&initial.sender).unwrap(); //TODO unwrap

        let val = RemoteVal(Outgoing {
            value: initial.payload,
            recipient: initial.recipient,
            sender: RemoteKey(sender),
            reply_serializer: reply_serializer as u8,
        });

        val
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Outgoing {
    pub(crate) value: Box<[u8]>,
    pub(crate) recipient: RemoteKey,
    pub(crate) sender: RemoteKey,
    /// Serialized to be used if a reply is sent
    pub(crate) reply_serializer: u8,
}

#[derive(Debug)]
pub(crate) struct Incoming {
    pub(crate) value: Box<[u8]>,
    pub(crate) sender: Address,
    /// Despite the key being a `RemoteKey`, this is a local key
    pub(crate) recipient: RemoteKey,
}
