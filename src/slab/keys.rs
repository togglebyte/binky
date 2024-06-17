use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::serializer::Serializer;

/// A serialized instance of an `AgentKey`.
/// Only `AgentKey`s should be serialized and deserialized
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub(crate) struct RemoteKey(#[serde(with = "serde_bytes")] pub(crate) Box<[u8]>);

impl RemoteKey {
    pub(crate) fn to_key(&self, serializer: Serializer) -> AgentKey {
        serializer
            .deserialize(&self.0)
            .expect("a remote key can always be deserialized")
    }
}

/// A key for accessing a value in a Slab<T>
#[derive(Copy, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub(crate) struct Key<T>(pub(super) u64, T);

#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub(crate) struct Base;
pub(crate) type BaseKey = Key<Base>;

impl Key<Base> {
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

impl From<BridgeKey> for BaseKey {
    fn from(value: BridgeKey) -> Self {
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

impl<T> Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} | {}>", self.index(), self.gen())
    }
}

impl<T: Default> From<u64> for Key<T> {
    fn from(value: u64) -> Self {
        Self(value, T::default())
    }
}

// /// A key for an agent in a slab
// #[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
// pub(crate) struct AgentKey(Key);
#[derive(Debug, Default, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub(crate) struct Agent;
/// A key for an agent in a slab
pub type AgentKey = Key<Agent>;

impl From<BaseKey> for AgentKey {
    fn from(value: BaseKey) -> Self {
        Key(value.0, Agent)
    }
}

// /// A key for a bridged connection (reader / writer pair) in a slab
// #[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
// pub(crate) struct BridgeKey(Key);

/// A key for a bridged connection (reader / writer pair) in a slab
#[derive(Debug, Default, Copy, Clone, PartialEq, Hash, Eq)]
pub(crate) struct Bridge;
pub(crate) type BridgeKey = Key<Bridge>;

impl From<Key<Agent>> for Key<Bridge> {
    fn from(value: Key<Agent>) -> Self {
        Self(value.0, Bridge)
    }
}

// impl Deref for BridgeKey {
//     type Target = Key;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl From<BridgeKey> for u64 {
//     fn from(key: BridgeKey) -> u64 {
//         key.0 .0
//     }
// }

// impl From<u64> for BridgeKey {
//     fn from(value: u64) -> Self {
//         Self(value.into())
//     }
// }

// impl From<AgentKey> for BridgeKey {
//     fn from(value: AgentKey) -> Self {
//         Self(value.into())
//     }
// }

#[derive(Debug, Default, Copy, Clone, PartialEq, Hash, Eq)]
pub(crate) struct Session;
pub type SessionKey = Key<Session>;
