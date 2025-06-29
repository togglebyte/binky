use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::agent::{SessionAgent, WriterAgent};
use crate::bridge::{SessionMessage, WriterMessage};
use crate::error::Result;
use crate::queue::Queue;
use crate::storage::Key;
use crate::{Address, Agent};

#[repr(u8)]
#[derive(Debug, Copy, Clone, Default)]
pub(crate) enum SessionNegotiation {
    HasExistingSession = 1,
    RequestNewSession = 2,
    ValidSession = 3,
    InvalidSession = 4,

    #[default]
    InvalidSessionNegotiation,
}

impl From<u8> for SessionNegotiation {
    fn from(value: u8) -> Self {
        match value {
            1 => SessionNegotiation::HasExistingSession,
            2 => SessionNegotiation::RequestNewSession,
            3 => SessionNegotiation::ValidSession,
            4 => SessionNegotiation::InvalidSession,
            _ => SessionNegotiation::InvalidSessionNegotiation,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Expiration {
    Never,
    After(Duration, Instant),
}

impl Expiration {
    pub fn never() -> Self {
        Self::Never
    }

    pub fn after(duration: Duration) -> Self {
        Self::After(duration, Instant::now())
    }

    pub(crate) fn renew(&mut self) {
        match self {
            Expiration::Never => (),
            Expiration::After(_, instant) => *instant = Instant::now(),
        }
    }

    pub(crate) fn has_expired(&self) -> bool {
        match self {
            Expiration::Never => false,
            Expiration::After(dur, ts) => ts.elapsed() > *dur,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum State {
    Write,
    Log,
}

#[derive(Debug)]
pub(crate) struct Session {
    expiration: Expiration,
    agent: SessionAgent,
    backlog: Queue<WriterMessage>,
    writer_key: Key,
    state: State,
}

impl Session {
    pub(crate) fn new(expiration: Expiration, agent: SessionAgent, writer_key: Key, cap: usize) -> Self {
        Self {
            expiration,
            agent,
            backlog: Queue::with_capacity(cap),
            writer_key,
            state: State::Write,
        }
    }

    fn push(&mut self, msg: WriterMessage) {
        self.backlog.push(msg);
    }

    pub(crate) async fn run(mut self) -> Result<()> {
        info!(
            "running session on {:?} -> writer {:?}",
            self.agent.key(),
            self.writer_key
        );
        while let Ok(msg) = self.agent.recv().await {
            match msg {
                SessionMessage::Writer(writer_msg) => match self.state {
                    State::Write => {
                        info!(
                            "session message received on {:?} -> writer {:?}",
                            self.agent.key(),
                            self.writer_key
                        );
                        self.agent.send(self.writer_key, writer_msg).await?
                    }
                    State::Log => drop(self.backlog.push(writer_msg)),
                },
                SessionMessage::AgentRemoved(key) if self.writer_key == key => {
                    tracing::info!("session is awaiting reconnect to {:?}", self.agent.key());
                    self.state = State::Log;
                    self.expiration.renew();
                }
                SessionMessage::AgentRemoved(_) => continue,
                SessionMessage::WriterReturned(new_key) => {
                    tracing::info!("session {:?} reactivated with {new_key:?}", self.agent.key());
                    self.state = State::Write;
                    self.writer_key = new_key;
                    while let Some(msg) = self.backlog.pop() {
                        self.agent.send(self.writer_key, msg).await?;
                    }
                }
                SessionMessage::CloseWriter => {
                    tracing::warn!("=============  here we close the writer");
                    self.agent.remove_writer(self.writer_key).await?;
                }
                SessionMessage::SessionPing => {
                    // If the session has expired reply with `Expired`.
                    // If the session is gone the router should receive
                    // and error when trying to send and thus we can remove the session

                    if let State::Write = self.state {
                        continue;
                    }

                    if self.expiration.has_expired() {
                        tracing::info!("removing session {:?}", self.agent.key());
                        self.agent.remove_self().await;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn key(&self) -> Key {
        self.agent.key()
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
pub(super) struct SessionPingAddress;

#[derive(Debug, Copy, Clone, Serialize)]
pub(super) struct PingLoop;

#[derive(Debug, Serialize, Deserialize)]
pub(super) enum PingControl {
    Stop,
    Start,
    Tick,
}

pub(super) async fn session_ping(mut agent: Agent) -> Result<()> {
    let mut ticker_address = run_ticker(&agent).await?;

    while let Ok(msg) = agent.recv::<PingControl>().await {
        match msg {
            crate::AgentMessage::Value { value, sender } => match value {
                PingControl::Stop => {
                    agent.remove_agent(ticker_address.clone()).await;
                }
                PingControl::Start => {
                    ticker_address = run_ticker(&agent).await?;
                }
                PingControl::Tick => agent.cleanup_sessions().await?,
            },
            crate::AgentMessage::Request { request, sender } => todo!(),
            crate::AgentMessage::AgentRemoved(address) => todo!(),
            crate::AgentMessage::Disconnected => todo!(),
            crate::AgentMessage::Connected => todo!(),
        }
    }

    Ok(())
}

async fn run_ticker(agent: &Agent) -> Result<Address> {
    let ticker = agent.agent(PingLoop).await?;
    let ticker_address = ticker.address();
    let address = agent.address();

    tokio::spawn(async move {
        let mut timer = crate::timeout().forever().duration(Duration::from_secs(60 * 5));
        loop {
            timer.sleep().await;
            ticker.send(&address, PingControl::Tick).await;
        }
    });

    Ok(ticker_address)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::serializer::Serializer;
    use crate::Router;

    #[tokio::test]
    async fn expire_session() {
        let mut router = Router::new();
        let router_ctx = router.ctx();
        tokio::spawn(router.run());
        let serializer = Serializer::Json;

        let session_agent = router_ctx.new_session_agent("session", None, serializer).await.unwrap();

        let writer_agent = router_ctx.new_writer_agent("writer", None, serializer).await.unwrap();

        let bridge_key = writer_agent.key();

        session_agent.track(writer_agent.key()).await;
        let expiration = Expiration::after(Duration::from_secs(60 * 5));
        let session = Session::new(expiration, session_agent, bridge_key, 100);
    }
}
