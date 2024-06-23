use std::fmt::{self, Debug};

use serde::{Deserialize, Serialize};

use crate::serializer::Serializer;

/// A serialized instance of an `AgentKey`.
/// Only `AgentKey`s should be serialized and deserialized
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct RemoteKey(#[serde(with = "serde_bytes")] pub(crate) Box<[u8]>);

impl RemoteKey {
    pub(crate) fn to_key(&self, serializer: Serializer) -> AgentKey {
        serializer
            .deserialize(&self.0)
            .expect("a remote key can always be deserialized")
    }
}

/// A key for accessing a value in a Slab<T>
#[derive(Copy, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct Key<T>(pub(super) u64, T);

#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub(crate) struct Base;
pub(crate) type BaseKey = Key<Base>;

impl Key<Base> {
    #[cfg(test)]
    pub(super) const ZERO: Self = Self(0, Base);

    pub(crate) fn new(inner: u64) -> Self {
        Self(inner, Base)
    }
}

impl From<AgentKey> for BaseKey {
    fn from(value: AgentKey) -> Self {
        Key(value.0, Base)
    }
}

impl From<WriterKey> for BaseKey {
    fn from(value: WriterKey) -> Self {
        Key(value.0, Base)
    }
}

impl From<SessionKey> for BaseKey {
    fn from(value: SessionKey) -> Self {
        Key(value.0, Base)
    }
}

impl<T> Key<T> {
    pub(super) fn bump(mut self) -> Self {
        let gen = self.gen().wrapping_add(1);
        self.set_gen(gen);
        self
    }

    pub(super) fn index(&self) -> usize {
        (self.0 << 16 >> 16) as usize
    }

    pub(super) fn gen(&self) -> u16 {
        (self.0 >> 48) as u16
    }

    #[cfg(test)]
    pub(super) fn set_index(&mut self, new_index: u64) {
        new_index << 16 >> 16;
        self.0 |= new_index;
    }

    pub(super) fn set_gen(&mut self, new_gen: u16) {
        let gen = (new_gen as u64) << 48;
        self.0 |= gen;
    }

    pub(crate) fn consume(self) -> u64 {
        self.0
    }
}

impl<T: Debug> Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{:?} {} | {}>", self.1, self.index(), self.gen())
    }
}

impl<T: Default> From<u64> for Key<T> {
    fn from(value: u64) -> Self {
        Self(value, T::default())
    }
}

impl<T> From<Key<T>> for u64 {
    fn from(key: Key<T>) -> Self {
        key.0
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub struct Agent;

/// A key for an agent in a slab
pub type AgentKey = Key<Agent>;

impl From<BaseKey> for AgentKey {
    fn from(value: BaseKey) -> Self {
        Key(value.0, Agent)
    }
}

impl From<WriterKey> for AgentKey {
    fn from(value: WriterKey) -> Self {
        Key(value.0, Agent)
    }
}

impl From<SessionKey> for AgentKey {
    fn from(value: SessionKey) -> Self {
        Key(value.0, Agent)
    }
}

/// A key for a bridged connection (reader / writer pair) in a slab
#[derive(Debug, Default, Copy, Clone, PartialEq, Hash, Eq)]
pub struct Writer;
pub type WriterKey = Key<Writer>;

impl From<Key<Agent>> for Key<Writer> {
    fn from(value: Key<Agent>) -> Self {
        Self(value.0, Writer)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Hash, Eq)]
pub struct Session;
/// A key to a session in the router
pub type SessionKey = Key<Session>;

impl Key<Session> {
    pub fn to_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl From<Key<Writer>> for Key<Session> {
    fn from(value: Key<Writer>) -> Self {
        Self(value.0, Session)
    }
}

impl From<Key<Agent>> for Key<Session> {
    fn from(value: Key<Agent>) -> Self {
        Self(value.0, Session)
    }
}
