use serde::{Deserialize, Serialize};

use crate::slab::{BridgeKey, RemoteKey};
use crate::value::{Initial, Outgoing, RemoteVal};

/// Message deserialized by the reading half of a socket
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum ReaderMessage {
    Value(Outgoing),
    AddressRequest {
        callback: u64,
        address: Box<[u8]>,
    },
    AddressResponse {
        callback: u64,
        address: Option<RemoteKey>,
    },
}

/// Any message sent to the writing half of a socket
pub(crate) enum WriterMessage {
    Value(RemoteVal<Initial>),
    AddressRequest {
        callback: u64,
        address: Box<[u8]>,
    },
    AddressResponse {
        callback: u64,
        address: Option<RemoteKey>,
    },
}

pub(crate) enum BridgeMessage {
    Writer(WriterMessage),
    AgentRemoved(BridgeKey),
    WriterReturned(BridgeKey),
    SessionPing,
}
