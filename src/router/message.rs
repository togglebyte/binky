
use flume::{bounded, Receiver, Sender};

use crate::address::InternalAddress;
use crate::error::Result;
use crate::request::{CallbackValue, LocalResponse, Pending, Request};
use crate::serializer::Serializer;
use crate::storage::{Key, KeyKind, RemoteKey};
use crate::value::{AnyValue, Incoming, Initial, RemoteVal};
use crate::Agent;

use super::Session;

pub(crate) enum RouterMessage {
    /// Send a value to a local agent
    Value {
        sender: Key,
        recipient: Key,
        value: AnyValue,
    },
    /// An agent sending a value to a remote via a bridge.
    OutgoingRemoteValue(RemoteVal<Initial>),
    /// An agent receiving a value from a remote via a bridge.
    IncomingRemoteValue(RemoteVal<Incoming>),
    /// Get the serializer for an agent
    GetSerializer {
        key: Key,
        reply: Sender<Result<Serializer>>,
    },
    /// Make a local request
    LocalRequest {
        sender: Key,
        recipient: Key,
        request: Request<Pending>,
    },
    /// Request a local address
    ResolveLocal {
        reply: Sender<Result<Key>>,
        address: Box<[u8]>,
    },
    /// Request a remote address
    ResolveRemote {
        reply: Sender<Result<RemoteKey>>,
        session: Key,
        address: Box<[u8]>,
    },
    /// Reader responds to writer
    RespondResolveRemote {
        callback: u64,
        address: Option<Key>,
        writer: Key,
    },
    /// Shutdown the router
    Shutdown,
    /// Request a new agent
    NewAgent {
        reply: Sender<Agent>,
        cap: Option<usize>,
        address: Option<Box<[u8]>>,
        serializer: Serializer,
        kind: KeyKind,
    },
    /// Remove an agent
    RemoveAgent(Key),
    /// Issue a callback
    Callback {
        callback_id: u64,
        callback_value: CallbackValue,
    },
    Track {
        tracker: Key,
        target: Key,
    },
    SessionExists(Key, Sender<bool>),
    RemoveWriter(Key),
}

impl RouterMessage {
    /// Create a message wrapping a Value message
    pub(crate) fn value(sender: Key, recipient: Key, value: AnyValue) -> Self {
        Self::Value {
            recipient,
            value,
            sender,
        }
    }

    /// Create a message wrapping a Value message
    pub(crate) fn remote_value(value: RemoteVal<Initial>) -> Self {
        Self::OutgoingRemoteValue(value)
    }

    /// Create a message wrapping a Value message
    pub(crate) fn incoming(value: Box<[u8]>, sender: InternalAddress, recipient: RemoteKey) -> Self {
        Self::IncomingRemoteValue(RemoteVal(Incoming {
            value,
            sender,
            recipient,
        }))
    }

    pub(crate) fn get_serializer(key: Key) -> (Receiver<Result<Serializer>>, Self) {
        let (tx, rx) = bounded(0);
        let msg = Self::GetSerializer {
            key,
            reply: tx,
        };
        (rx, msg)
    }

    /// Create a request
    pub(crate) fn local_request(
        sender: Key,
        recipient: Key,
        value: AnyValue,
    ) -> (LocalResponse, Self) {
        let (request, response) = Request::new(value);

        let msg = RouterMessage::LocalRequest {
            recipient,
            request,
            sender,
        };

        (response, msg)
    }

    /// Shutdown router
    pub(crate) fn shutdown() -> Self {
        Self::Shutdown
    }

    /// Resolve a local address
    pub(crate) fn resolve_local(address: Box<[u8]>) -> (Receiver<Result<Key>>, Self) {
        let (tx, rx) = flume::bounded(0);
        let msg = RouterMessage::ResolveLocal { reply: tx, address };

        (rx, msg)
    }

    /// Resolve a remote address
    pub(crate) fn resolve_remote(
        session: Key,
        address: Box<[u8]>,
    ) -> (Receiver<Result<RemoteKey>>, Self) {
        let (tx, rx) = flume::bounded(0);
        let msg = RouterMessage::ResolveRemote {
            reply: tx,
            session,
            address,
        };

        (rx, msg)
    }

    /// Response to a remote resolution request
    pub(crate) fn respond_resolve_remote(
        callback: u64,
        address: Option<Key>,
        writer: Key,
    ) -> Self {
        Self::RespondResolveRemote {
            callback,
            address,
            writer,
        }
    }

    pub(crate) fn new_agent(
        address: Option<Box<[u8]>>,
        cap: Option<usize>,
        serializer: Serializer,
        kind: KeyKind,
    ) -> (Receiver<Agent>, RouterMessage) {
        let (tx, rx) = flume::bounded(0);
        let msg = Self::NewAgent {
            reply: tx,
            address,
            cap,
            serializer,
            kind,
        };

        (rx, msg)
    }

    pub(crate) fn remove_agent(key: Key) -> RouterMessage {
        Self::RemoveAgent(key)
    }
}
