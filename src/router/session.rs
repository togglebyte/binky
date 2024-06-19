use std::time::{Duration, Instant};

use crate::agent::BridgeAgent;
use crate::bridge::BridgeMessage;
use crate::bridge::WriterMessage;
use crate::error::Result;
use crate::queue::Queue;
use crate::slab::BridgeKey;
use crate::SessionKey;

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

pub(crate) struct Session {
    expiration: Expiration,
    agent: BridgeAgent,
    backlog: Queue<BridgeMessage>,
    writer_key: BridgeKey,
    state: State,
}

impl Session {
    pub(crate) fn new(
        expiration: Expiration,
        agent: BridgeAgent,
        writer_key: BridgeKey,
        cap: usize,
    ) -> Self {
        Self {
            expiration,
            agent,
            backlog: Queue::with_capacity(cap),
            writer_key,
            state: State::Write,
        }
    }

    fn push(&mut self, msg: BridgeMessage) {
        self.backlog.push(msg);
    }

    pub(crate) async fn run(mut self) -> Result<()> {
        while let Ok(msg) = self.agent.recv().await {
            match msg {
                BridgeMessage::Writer(_) => match self.state {
                    State::Write => self.agent.send(self.writer_key, msg).await?,
                    State::Log => {
                        self.backlog.push(msg);
                        continue;
                    }
                },
                BridgeMessage::AgentRemoved(key) if self.writer_key == key => {
                    self.state = State::Log;
                    self.expiration.renew();
                }
                BridgeMessage::AgentRemoved(key) => continue,
                BridgeMessage::WriterReturned(new_key) => {
                    self.writer_key = new_key;
                    while let Some(msg) = self.backlog.pop() {
                        self.agent.send(self.writer_key, msg).await?;
                    }
                }
                BridgeMessage::SessionPing => {
                    // If the session has expired reply with `Expired`.
                    // If the session is gone the router should receive
                    // and error when trying to send and thus we can remove the session

                    if let State::Write = self.state {
                        continue;
                    }

                    if self.expiration.has_expired() {
                        self.agent.remove_self();
                        break
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn key(&self) -> SessionKey {
        self.agent.key().into()
    }
}
