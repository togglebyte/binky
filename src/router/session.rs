use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::agent::BridgeAgent;
use crate::bridge::WriterMessage;
use crate::slab::{AgentKey, BridgeKey, SessionKey};

pub enum Kind {
    //
}

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

pub(crate) struct Session {
    expiration: Expiration,
    writer: BridgeAgent,
    backlog: VecDeque<WriterMessage>,
}

impl Session {
    pub fn new(expiration: Expiration, writer: BridgeAgent) -> Self {
        Self {
            expiration,
            writer,
            backlog: VecDeque::new(),
        }
    }
}
