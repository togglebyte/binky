use std::fmt::{self, Debug};
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::serializer::Serializer;

/// A key for accessing a value in a Slab<T>
#[derive(Copy, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
#[repr(transparent)]
pub(crate) struct Key(pub(super) u64);

/// A serialized instance of a `Key`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub(crate) struct RemoteKey(#[serde(with = "serde_bytes")] pub(crate) Box<[u8]>);

impl RemoteKey {
    pub(crate) fn to_key(&self, serializer: Serializer) -> Key {
        serializer
            .deserialize(&self.0)
            .expect("a remote key can always be deserialized")
    }
}

impl Key {
    pub(super) const ZERO: Self = Self(0);

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
}

impl Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} | {}>", self.index(), self.gen())
    }
}

impl From<u64> for Key {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Key> for u64 {
    fn from(key: Key) -> Self {
        key.0
    }
}

/// A key for an agent in a slab
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub(crate) struct AgentKey(Key);

impl Deref for AgentKey {
    type Target = Key;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Key> for AgentKey {
    fn from(key: Key) -> Self {
        Self(key)
    }
}

impl From<u64> for AgentKey {
    fn from(key: u64) -> Self {
        Self(key.into())
    }
}

impl From<AgentKey> for u64 {
    fn from(key: AgentKey) -> Self {
        key.0 .0
    }
}

impl From<AgentKey> for Key {
    fn from(key: AgentKey) -> Self {
        key.0
    }
}

impl From<BridgeKey> for Key {
    fn from(key: BridgeKey) -> Self {
        key.0
    }
}

/// A key for a bridged connection (reader / writer pair) in a slab
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BridgeKey(Key);

impl Deref for BridgeKey {
    type Target = Key;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<BridgeKey> for u64 {
    fn from(key: BridgeKey) -> u64 {
        key.0 .0
    }
}

impl From<u64> for BridgeKey {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}

impl From<AgentKey> for BridgeKey {
    fn from(value: AgentKey) -> Self {
        Self(value.into())
    }
}
