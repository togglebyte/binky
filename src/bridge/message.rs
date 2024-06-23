use serde::{Deserialize, Serialize};

use crate::slab::{WriterKey, RemoteKey};
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
#[derive(Debug)]
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
    Shutdown
}

#[derive(Debug)]
pub(crate) enum SessionMessage {
    // Message to forward to the associated `Writer`
    Writer(WriterMessage),
    AgentRemoved(WriterKey),
    WriterReturned(WriterKey),
    SessionPing,
    CloseWriter,
}
