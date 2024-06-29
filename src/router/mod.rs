#[deny(missing_docs)]
use std::collections::HashMap;

use flume::{bounded, unbounded, Receiver, Sender};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::info;

pub(crate) use self::message::RouterMessage;
pub(crate) use self::session::{Expiration, Session};
use crate::address::InternalAddress;
use crate::agent::{AnyMessage, SessionAgent, WriterAgent};
use crate::bridge::{Bridge, SessionMessage, WriterMessage};
use crate::error::{Error, Result};
use crate::request::{Callback, CallbackValue, Pending, Request};
use crate::serializer::Serializer;
use crate::storage::{Agents, Key, KeyKind, RemoteKey, Slab};
use crate::value::{AnyValue, Incoming, Initial, RemoteVal};
use crate::{Agent, Listener};

mod message;
pub(crate) mod session;

#[derive(Debug, Clone)]
pub(crate) struct RouterCtx {
    tx: Sender<RouterMessage>,
    serializer: Serializer,
}

impl RouterCtx {
    pub(crate) async fn send(&self, message: RouterMessage) -> Result<()> {
        self.tx.send_async(message).await?;
        Ok(())
    }

    pub(crate) fn send_sync(&self, message: RouterMessage) -> Result<()> {
        self.tx.send(message)?;
        Ok(())
    }

    pub(crate) fn serialize(&self, data: &impl Serialize) -> Result<Box<[u8]>> {
        self.serializer.serialize(data)
    }

    pub(crate) fn deserialize<T: DeserializeOwned>(&self, data: impl AsRef<[u8]>) -> Result<T> {
        self.serializer.deserialize(data)
    }

    pub(crate) fn serializer(&self) -> Serializer {
        self.serializer
    }

    pub(crate) async fn new_agent<A: Serialize>(
        &self,
        address: impl Into<Option<A>>,
        cap: Option<usize>,
        serializer: Serializer,
        kind: KeyKind,
    ) -> Result<Agent> {
        let address = match &address.into() {
            Some(addr) => Some(self.serializer.serialize(&addr)?),
            None => None,
        };
        let (rx, msg) = RouterMessage::new_agent(address, cap, serializer, kind);
        self.tx.send_async(msg).await?;
        Ok(rx.recv_async().await?)
    }

    pub(crate) async fn new_writer_agent<A: Serialize>(
        &self,
        address: impl Into<Option<A>>,
        cap: Option<usize>,
        serializer: Serializer,
    ) -> Result<WriterAgent> {
        let agent = self.new_agent(address, cap, serializer, KeyKind::Writer).await?;
        Ok(WriterAgent::new(agent))
    }

    pub(crate) async fn new_session_agent<A: Serialize>(
        &self,
        address: impl Into<Option<A>>,
        cap: Option<usize>,
        serializer: Serializer,
    ) -> Result<SessionAgent> {
        dbg!("not parsnip");
        let agent = self.new_agent(address, cap, serializer, KeyKind::Session).await?;
        Ok(SessionAgent::new(agent))
    }

    pub(crate) async fn lookup_address(&self, address: Box<[u8]>) -> Result<Key> {
        let (rx, msg) = RouterMessage::resolve_local(address);
        self.tx.send_async(msg).await?;
        rx.recv_async().await?
    }

    pub(crate) async fn callback(&self, callback_id: u64, callback_value: CallbackValue) -> Result<()> {
        let msg = RouterMessage::Callback {
            callback_id,
            callback_value,
        };
        self.tx.send_async(msg).await?;
        Ok(())
    }

    pub(crate) async fn track(&self, tracker: Key, target: Key) -> Result<()> {
        let msg = RouterMessage::Track { tracker, target };
        Ok(self.tx.send_async(msg).await?)
    }

    pub(crate) async fn remove(&self, key: Key) -> Result<()> {
        let msg = RouterMessage::RemoveAgent(key);
        self.tx.send_async(msg).await?;
        Ok(())
    }

    pub(crate) async fn session_exists(&self, session_key: Key) -> bool {
        let (tx, rx) = flume::bounded(0);
        let msg = RouterMessage::SessionExists(session_key, tx);
        let _ = self.tx.send_async(msg).await;

        rx.recv_async().await.unwrap_or(false)
    }

    pub(crate) async fn session_track_writer(&self, session: Key, writer: Key) -> Result<()> {
        let msg = RouterMessage::Track {
            tracker: session.into(),
            target: writer.into(),
        };
        self.tx.send_async(msg).await?;
        Ok(())
    }

    pub(crate) async fn remove_writer(&self, writer_key: Key) -> Result<()> {
        let msg = RouterMessage::RemoveWriter(writer_key);
        self.tx.send_async(msg).await?;
        Ok(())
    }
}

#[derive(Debug)]
struct AgentEntry {
    tx: Sender<AnyMessage>,
    address: Option<Box<[u8]>>,
    serializer: Serializer,
}

impl AgentEntry {
    fn serialize(&self, data: &impl Serialize) -> Option<Box<[u8]>> {
        self.serializer.serialize(data).ok()
    }
}

/// A message router
#[derive(Debug)]
pub struct Router {
    agents: Agents<AgentEntry>,
    addresses: HashMap<Box<[u8]>, Key>,
    tracked_by: HashMap<Key, Vec<Key>>,
    sessions: HashMap<Key, ()>,
    router_tx: Sender<RouterMessage>,
    router_rx: Receiver<RouterMessage>,
    serializer: Serializer,
    callbacks: Slab<Callback>,
}

impl Router {
    fn ctx(&self) -> RouterCtx {
        RouterCtx {
            tx: self.router_tx.clone(),
            serializer: self.serializer,
        }
    }

    async fn remove_agent(&mut self, key: Key) {
        eprintln!("removing: {key:?}");
        let Some(agent) = self.agents.remove(key) else {
            return;
        };

        if let Some(bytes) = agent.address {
            self.addresses.remove(&bytes);
        }

        if let Some(tracked) = self.tracked_by.remove(&key) {
            eprintln!("notify: {tracked:?}");
            for agent in tracked {
                let Some(agent) = self.agents.get(agent.into()) else {
                    continue;
                };
                let _ = agent.tx.send_async(AnyMessage::AgentRemoved(key)).await;
            }
        }
    }

    /// Create a new instance of a message router:
    /// ```
    /// use binky::Router;
    /// let router = Router::new();
    /// ```
    pub fn new() -> Self {
        let (router_tx, router_rx) = unbounded();
        Self {
            agents: Agents::new(),
            addresses: HashMap::default(),
            tracked_by: HashMap::default(),
            sessions: HashMap::default(),
            router_tx,
            router_rx,
            serializer: Serializer::Json,
            callbacks: Slab::new(),
        }
    }

    /// Create a new agent
    /// ```
    /// use binky::Router;
    ///
    /// let mut router = Router::new();
    /// let agent = router.agent(1);
    /// ```
    fn new_agent(
        &mut self,
        address: Option<Box<[u8]>>,
        cap: Option<usize>,
        serializer: Serializer,
        kind: KeyKind,
    ) -> Agent {
        let (tx, rx) = match cap {
            Some(cap) => bounded(cap),
            None => unbounded(),
        };

        let entry = AgentEntry {
            tx,
            address: address.clone(),
            serializer,
        };

        let key = self.agents.insert(entry, kind);
        if let Some(address) = address {
            self.addresses.insert(address, key);
        }

        Agent::new(key.into(), self.ctx(), rx, self.serializer)
    }

    /// Create a new agent with an unsized capacity
    /// ```
    /// use binky::{Agent, Router};
    /// let mut router = Router::new();
    /// let agent = router.agent(1);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the address fails to be serialized
    pub fn agent(&mut self, address: impl Serialize) -> Agent {
        let address = self
            .serializer
            .serialize(&address)
            .expect("failed to serialize the address");
        self.new_agent(Some(address), None, self.serializer, KeyKind::Agent)
    }

    /// Create a new agent with an unsized capacity
    /// ```
    /// use binky::{Agent, Router};
    /// let mut router = Router::new();
    /// let agent = router.agent_with_capacity(1, 100);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the address fails to be serialized
    pub fn agent_with_capacity(&mut self, address: impl Serialize, cap: usize) -> Agent {
        let address = self
            .serializer
            .serialize(&address)
            .expect("failed to serialize the address");

        self.new_agent(Some(address), Some(cap), self.serializer, KeyKind::Agent)
    }

    /// Listen to incoming messages on a given listener.
    /// This will create a bridge between two routers.
    /// ```no_run
    /// use binky::{Router, TcpListener};
    /// use serde::{Deserialize, Serialize};
    ///
    /// # async fn async_run() {
    /// let mut router = Router::new();
    /// let listener = TcpListener::bind("127.0.0.1:8000").await.unwrap();
    /// router.listen(listener);
    /// router.run().await;
    /// # }
    /// ```
    pub fn listen(&mut self, listener: impl Listener + Sync) {
        let bridge = Bridge::new(listener, self.ctx(), None);
        // TODO should the handle be kept for when the router is shut down?
        tokio::spawn(bridge.run());
    }

    /// Start the router and run it to completion,
    /// taking ownership of the router.
    /// ```
    /// use binky::Router;
    /// # async fn async_run() {
    /// let mut router = Router::new();
    /// let agent = router.agent(());
    ///
    /// let handle = tokio::spawn(router.run());
    ///
    /// agent.shutdown().await;
    ///
    /// handle.await.unwrap();
    /// # }
    /// ```
    #[tracing::instrument]
    pub async fn run(mut self) -> Self {
        while self.next().await {}
        self
    }

    pub(crate) async fn next(&mut self) -> bool {
        if let Ok(msg) = self.router_rx.recv_async().await {
            match msg {
                RouterMessage::Value {
                    recipient,
                    value,
                    sender,
                } => self.value(recipient, value, sender).await,
                RouterMessage::OutgoingRemoteValue(value) => self.outgoing_remote_value(value).await,
                RouterMessage::IncomingRemoteValue(value) => self.incoming_remote_value(value).await,
                RouterMessage::LocalRequest {
                    sender,
                    recipient,
                    request,
                } => self.local_request(sender, recipient, request).await,
                RouterMessage::GetSerializer { key, reply } => {
                    let serializer = self.agents.get(key).map(|entry| entry.serializer);
                    reply.send_async(serializer.ok_or(Error::AddressNotFound)).await;
                }
                RouterMessage::Shutdown => return false,
                RouterMessage::ResolveLocal { reply, address } => {
                    let key = self
                        .addresses
                        .get(&*address)
                        .copied()
                        .map(Into::into)
                        .ok_or(Error::AddressNotFound);
                    reply.send_async(key).await;
                }
                RouterMessage::ResolveRemote {
                    reply,
                    session,
                    address,
                } => {
                    self.resolve_remote(reply, session, address).await;
                }
                RouterMessage::RespondResolveRemote {
                    callback,
                    address,
                    writer,
                } => self.respond_to_resolve_remote(callback, address, writer).await,
                RouterMessage::NewAgent {
                    reply,
                    cap,
                    address,
                    serializer,
                    kind,
                } => {
                    let agent = self.new_agent(address, cap, serializer, kind);
                    let _ = reply.send_async(agent).await;
                }
                RouterMessage::RemoveAgent(key) => {
                    info!("remove agent {key:?}");
                    self.remove_agent(key).await;
                }
                RouterMessage::Callback {
                    callback_id,
                    callback_value,
                } => self.callback(callback_id, callback_value).await,
                RouterMessage::Track { tracker, target } => {
                    info!("{tracker:?} is tracking {target:?}");
                    let targets = self.tracked_by.entry(target).or_default();
                    targets.push(tracker);
                }
                RouterMessage::SessionExists(key, tx) => {
                    info!("session exists query");
                    let _ = tx.send_async(self.agents.get(key.into()).is_some()).await;
                }
                RouterMessage::RemoveWriter(writer_key) => self.remove_writer(writer_key).await,
            }
        }

        true
    }

    async fn value(&mut self, recipient: Key, value: AnyValue, sender: Key) {
        let Some(agent) = self.agents.get(recipient) else {
            return;
        };
        // If this fails it means the channel was closed and
        // the only thing left to do here is to remove the sending half of the channel
        if let Err(_) = agent
            .tx
            .send_async(AnyMessage::Value {
                value,
                sender: InternalAddress::Local(sender),
            })
            .await
        {
            self.remove_agent(recipient).await;
        }
    }

    async fn outgoing_remote_value(&self, value: RemoteVal<Initial>) {
        let session = value.local_session_key;
        let Some(session) = self.agents.get(session.into()) else { return };

        info!("sending outgoing remote value");
        session
            .tx
            .send_async(AnyMessage::Session(SessionMessage::Writer(WriterMessage::Value(value))))
            .await;
    }

    async fn incoming_remote_value(&self, value: RemoteVal<Incoming>) {
        info!("incoming remote value");

        // TODO
        // if the agent isn't found reply with a NotFound error
        let agent_key = value.recipient.to_key(self.serializer);
        let Some(agent) = self.agents.get(agent_key.into()) else { return };

        let incoming = value.0;

        // There is nothing to do if the agent was removed but ignore the result
        let _ = agent
            .tx
            .send_async(AnyMessage::RemoteValue {
                value: incoming.value,
                sender: incoming.sender,
            })
            .await;
    }

    async fn local_request(&self, sender: Key, recipient: Key, request: Request<Pending>) {
        info!("local request");
        let Some(recipient) = self.agents.get(recipient.into()) else {
            request.reply(Err(Error::AddressNotFound)).await;
            return;
        };

        if let Err(e) = recipient
            .tx
            .send_async(AnyMessage::LocalRequest {
                request,
                sender: InternalAddress::Local(sender),
            })
            .await
        {
            let AnyMessage::LocalRequest { request, .. } = e.0 else { unreachable!() };
            request.reply(Err(Error::AddressNotFound)).await;
        }
    }

    async fn resolve_remote(&mut self, reply: Sender<Result<RemoteKey>>, session: Key, address: Box<[u8]>) {
        // panic!("this requires that callbacks have a different key kind");
        info!("resolve remote");
        let Some(session_agent) = self.agents.get(session.into()) else {
            reply.send_async(Err(Error::AddressNotFound)).await;
            return;
        };
        let callback = Callback::Resolve(reply);
        let callback = self.callbacks.insert(callback);

        let writer_msg = WriterMessage::AddressRequest { callback, address };

        if let Err(_e) = session_agent
            .tx
            .send_async(AnyMessage::Session(SessionMessage::Writer(writer_msg)))
            .await
        {
            self.remove_agent(session.into()).await;
        }
    }

    async fn respond_to_resolve_remote(&self, callback: u64, address: Option<Key>, writer: Key) {
        info!("respond resolve remote");
        let Some(bridge) = self.agents.get(writer.into()) else { return };

        let address = bridge.serialize(&address).map(RemoteKey);

        let writer_msg = WriterMessage::AddressResponse { callback, address };

        // If the rx end is closed there is nothing we can do here
        let _ = bridge
            .tx
            .send_async(AnyMessage::Session(SessionMessage::Writer(writer_msg)))
            .await;
    }

    async fn callback(&mut self, callback_id: u64, callback_value: CallbackValue) {
        info!("callback");
        let Some(cb) = self.callbacks.remove(callback_id) else { return };
        match (cb, callback_value) {
            (Callback::Resolve(tx), CallbackValue::Resolve(value)) => {
                // If the receiving end is removed there is nothing to do here
                let _ = tx.send_async(value).await;
            }
        }
    }

    async fn remove_writer(&mut self, key: Key) {
        info!("remove writer {key:?}");
        let Some(writer) = self.agents.remove(key.into()) else { return };
        if let Err(_e) = writer.tx.send_async(AnyMessage::Writer(WriterMessage::Shutdown)).await {
            self.remove_agent(key.into()).await;
        }
    }
}

#[cfg(test)]
mod test {
    use serde::Deserialize;

    use super::*;
    use crate::agent::AgentMessage;
    use crate::timeout;

    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    enum Address {
        A,
        B,
    }

    #[test]
    fn new_router() {
        let router = Router::new();
    }

    #[test]
    fn new_agent() {
        let mut router = Router::new();
        router.agent(Address::A);
    }

    #[tokio::test]
    async fn send_message_from_agent_to_agent() {
        let mut router = Router::new();
        let a = router.agent(Address::A);
        let mut b = router.agent(Address::B);

        tokio::spawn(router.run());

        let recipient = a.resolve(Address::B).await.unwrap();
        a.send(&recipient, "hello".to_string()).await;

        let AgentMessage::Value { value, sender } = b.recv::<String>().await.unwrap() else { panic!() };

        assert_eq!("hello".to_string(), value);
        assert_eq!(a.address(), sender);
    }

    #[tokio::test]
    async fn shutdown() {
        let mut router = Router::new();
        let a = router.agent(Address::A);

        let handle = tokio::spawn(router.run());

        a.shutdown().await;
        handle.await;
    }

    #[tokio::test]
    async fn resolve_local() {
        let mut router = Router::new();
        let a = router.agent(Address::A);
        let b = router.agent(Address::B);
        tokio::spawn(router.run());

        let recipient = a.resolve(Address::B).await.unwrap();
        assert_eq!(recipient, b.address());
    }

    #[tokio::test]
    async fn remove_agent() {
        let mut router = Router::new();
        let a = router.agent(Address::A);
        let _b = router.agent(Address::B);

        tokio::spawn(async move {
            a.remove_agent(a.address()).await;
            use crate::address::Address as A;
            let b_addr = a.resolve(Address::B).await.unwrap();
            a.remove_agent(b_addr).await;
            a.shutdown().await;
        });
        let router = router.run().await;

        assert_eq!(router.agents.count(), 0);
    }

    #[tokio::test]
    async fn request_response() {
        let mut router = Router::new();
        let adder = router.agent(Address::A);
        let mut b = router.agent(Address::B);

        let _ = tokio::spawn(router.run());

        let _ = tokio::spawn(async move {
            let recipient = adder.resolve(Address::B).await.unwrap();
            let response = adder.request::<u8>(&recipient, (1u8, 2u8)).await;
            assert_eq!(response.unwrap(), 3);
        });

        let AgentMessage::Request { request, .. } = b.recv::<String>().await.unwrap() else { panic!() };
        let request = request.read::<(u8, u8)>().unwrap();
        let (a, b) = *request;
        request.reply(a + b).await;
    }

    #[tokio::test]
    async fn track_agent() {
        let mut router = Router::new();
        let mut a = router.agent(Address::A);
        let b = router.agent(Address::B);

        let _ = tokio::spawn(router.run());

        // Agent A is waiting for Agent B to shut down
        let h1 = tokio::spawn(async move {
            let address = a
                .resolve_with_retry(Address::B, timeout().duration_ms(10).retries(10))
                .await
                .unwrap();

            a.track(&address).await.unwrap();

            let Ok(AgentMessage::AgentRemoved(addr)) = a.recv::<()>().await else { panic!() };
            assert_eq!(addr, address);
        });

        let h2 = tokio::spawn(async move {
            timeout().duration_ms(10).retries(1).sleep().await;
            b.remove_self().await
        });

        h1.await.unwrap();
        h2.await.unwrap();
    }
}
