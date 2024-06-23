use serde::Serialize;
use tracing::info;

use crate::address::InternalAddress;
use crate::bridge::{SessionMessage, WriterMessage};
use crate::error::Result;
use crate::serializer::Serializer;
use crate::slab::WriterKey;
use crate::{Agent, SessionKey};

#[derive(Debug)]
pub(crate) struct SessionAgent(Agent);

impl SessionAgent {
    pub(crate) fn new(agent: Agent) -> Self {
        Self(agent)
    }

    pub(crate) fn key(&self) -> SessionKey {
        self.0.key().into()
    }

    #[tracing::instrument]
    pub(crate) async fn recv(&self) -> Result<SessionMessage> {
        let msg = self.0.rx.recv_async().await?;
        match msg {
            super::AnyMessage::Session(value) => {
                info!("session message {value:?}");
                Ok(value)
            }
            // TODO what should be done here?
            // The bridge should perhaps just ignore
            // agent messages? Maybe a log entry?
            super::AnyMessage::RemoteValue { .. } => {
                unreachable!("bridge agent should never get a remote value")
            }
            super::AnyMessage::Value { value, .. } => {
                info!("session message (downcast) {value:?}");
                Ok(*value
                .downcast::<SessionMessage>()
                .expect("only session messages can be sent to the writer"))
            }
            super::AnyMessage::LocalRequest { .. } => todo!("local request"),
            super::AnyMessage::AgentRemoved(key) => {
                info!("agent removed: {key:?}");
                Ok(SessionMessage::AgentRemoved(key.into()))
            },
            super::AnyMessage::Writer(_) => unreachable!("writer messages should not be sent directly to the session"),
        }
    }

    pub(crate) async fn send(&self, recipient: WriterKey, msg: WriterMessage) -> Result<()> {
        let address = InternalAddress::Local(recipient.consume().into()).into();
        self.0.send_local(&address, msg).await
    }

    pub(crate) fn serializer(&self) -> Serializer {
        self.0.serializer
    }

    pub(crate) fn serialize(&self, value: &impl Serialize) -> Result<Box<[u8]>> {
        self.0.serializer.serialize(value)
    }

    pub(crate) async fn track(&self, key: WriterKey) -> Result<()> {
        let addr = InternalAddress::Local(key.into()).into();
        self.0.track(&addr).await
    }

    pub(crate) async fn remove_self(self) {
        let _ = self.0.remove_self().await;
    }

    pub(crate) async fn remove_writer(&self, writer_key: WriterKey) -> Result<()> {
        self.0
            .remove_agent(InternalAddress::Local(writer_key.into()).into())
            .await
    }
}
