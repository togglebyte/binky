use serde::Serialize;

use crate::bridge::WriterMessage;
use crate::error::Result;
use crate::serializer::Serializer;
use crate::slab::BridgeKey;
use crate::value::{Initial, RemoteVal};
use crate::Agent;

pub(crate) struct BridgeAgent(Agent);

impl BridgeAgent {
    pub(crate) fn new(agent: Agent) -> Self {
        Self(agent)
    }

    pub(crate) fn key(&self) -> BridgeKey {
        self.0.key().into()
    }

    //pub(crate) async fn recv(&self) -> Result<RemoteVal<Initial>> {
    pub(crate) async fn recv(&self) -> Result<WriterMessage> {
        match self.0.rx.recv_async().await? {
            super::AnyMessage::Bridge(value) => Ok(value),
            // TODO what should be done here? The
            // bridge should perhaps just ignore agent
            // messages? Maybe a log entry?
            super::AnyMessage::Value { .. }
            | super::AnyMessage::RemoteValue { .. }
            | super::AnyMessage::LocalRequest { .. } => todo!(),
        }
    }

    pub(crate) fn serializer(&self) -> Serializer {
        self.0.serializer
    }

    pub(crate) fn serialize(&self, value: &impl Serialize) -> Result<Box<[u8]>> {
        self.0.serializer.serialize(value)
    }
}
