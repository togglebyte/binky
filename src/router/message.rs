
use flume::{bounded, Receiver, Sender};

use crate::address::InternalAddress;
use crate::error::Result;
use crate::request::{CallbackValue, LocalResponse, Pending, Request};
use crate::serializer::Serializer;
use crate::slab::{AgentKey, WriterKey, RemoteKey};
use crate::value::{AnyValue, Incoming, Initial, RemoteVal};
use crate::{Agent, SessionKey};

use super::Session;

pub(crate) enum RouterMessage {
    /// Send a value to a local agent
    Value {
        sender: AgentKey,
        recipient: AgentKey,
        value: AnyValue,
    },
    /// An agent sending a value to a remote via a bridge.
    OutgoingRemoteValue(RemoteVal<Initial>),
    /// An agent receiving a value from a remote via a bridge.
    IncomingRemoteValue(RemoteVal<Incoming>),
    /// Get the serializer for an agent
    GetSerializer {
        key: u64,
        reply: Sender<Result<Serializer>>,
    },
    /// Make a local request
    LocalRequest {
        sender: AgentKey,
        recipient: AgentKey,
        request: Request<Pending>,
    },
    /// Request a local address
    ResolveLocal {
        reply: Sender<Result<AgentKey>>,
        address: Box<[u8]>,
    },
    /// Request a remote address
    ResolveRemote {
        reply: Sender<Result<RemoteKey>>,
        session: SessionKey,
        address: Box<[u8]>,
    },
    /// Reader responds to writer
    RespondResolveRemote {
        callback: u64,
        address: Option<AgentKey>,
        writer: SessionKey,
    },
    /// Shutdown the router
    Shutdown,
    /// Request a new agent
    NewAgent {
        reply: Sender<Agent>,
        cap: Option<usize>,
        address: Option<Box<[u8]>>,
        serializer: Serializer,
    },
    /// Remove an agent
    RemoveAgent(AgentKey),
    /// Issue a callback
    Callback {
        callback_id: u64,
        callback_value: CallbackValue,
    },
    Track {
        tracker: AgentKey,
        target: AgentKey,
    },
    SessionExists(SessionKey, Sender<bool>),
    CleanupSessions,
    RemoveWriter(WriterKey),
}

impl RouterMessage {
    /// Create a message wrapping a Value message
    pub(crate) fn value(sender: AgentKey, recipient: AgentKey, value: AnyValue) -> Self {
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

    pub(crate) fn get_serializer(key: SessionKey) -> (Receiver<Result<Serializer>>, Self) {
        let (tx, rx) = bounded(0);
        let msg = Self::GetSerializer {
            key: key.consume(),
            reply: tx,
        };
        (rx, msg)
    }

    /// Create a request
    pub(crate) fn local_request(
        sender: AgentKey,
        recipient: AgentKey,
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
    pub(crate) fn resolve_local(address: Box<[u8]>) -> (Receiver<Result<AgentKey>>, Self) {
        let (tx, rx) = flume::bounded(0);
        let msg = RouterMessage::ResolveLocal { reply: tx, address };

        (rx, msg)
    }

    /// Resolve a remote address
    pub(crate) fn resolve_remote(
        session: SessionKey,
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
        address: Option<AgentKey>,
        writer: SessionKey,
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
    ) -> (Receiver<Agent>, RouterMessage) {
        let (tx, rx) = flume::bounded(0);
        let msg = Self::NewAgent {
            reply: tx,
            address,
            cap,
            serializer,
        };

        (rx, msg)
    }

    pub(crate) fn remove_agent(key: AgentKey) -> RouterMessage {
        Self::RemoveAgent(key)
    }
}
