use std::fmt::{self, Debug};
use std::mem::swap;

use serde::{Deserialize, Serialize};

use crate::serializer::Serializer;

const INDEX: usize = 48;
const GEN: usize = 14;
const KIND: usize = 2;
const INDEX_MASK: u64 = 0x0000FFFFFFFFFFFF;
const GEN_MASK: u64 = 0xCFFFFFFFFFFFFFFF;
const KIND_MASK: u64 = 0x3FFFFFFFFFFFFFFF;

/// A serialized instance of an `AgentKey`.
/// Only `AgentKey`s should be serialized and deserialized
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct RemoteKey(#[serde(with = "serde_bytes")] pub(crate) Box<[u8]>);

impl RemoteKey {
    pub(crate) fn to_key(&self, serializer: Serializer) -> Key {
        serializer
            .deserialize(&self.0)
            .expect("a remote key can always be deserialized")
    }
}

/// A key for accessing a value in Agents<T>
/// They key is a single u64 where only the first 48 bits represent
/// the index, followed by
#[derive(Copy, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct Key(pub(super) u64);

impl Key {
    const ZERO: Self = Self::new(0, KeyKind::Agent);

    pub const fn new(mut index: u64, kind: KeyKind) -> Self {
        index = index | (kind as u64) << INDEX + GEN;
        Self(index)
    }

    pub const fn kind(&self) -> KeyKind {
        match (self.0 >> INDEX + GEN) as u8 {
            1 => KeyKind::Agent,
            2 => KeyKind::Writer,
            3 => KeyKind::Session,
            _ => panic!("invalid key"),
        }
    }

    pub(super) fn bump(&mut self) {
        let gen = self.gen().wrapping_add(1);
        self.set_gen(gen);
    }

    #[cfg(test)]
    pub(super) fn set_index(&mut self, new_index: u64) {
        // Clear the generation and kind bits
        new_index << GEN + KIND >> GEN + KIND;
        self.0 |= new_index;
    }

    pub(super) fn set_gen(&mut self, mut new_gen: u16) {
        // Clear the `kind` bits of the gen
        self.0 &= GEN_MASK;
        self.0 |= (new_gen as u64) << INDEX;
    }

    fn set_kind(&mut self, kind: KeyKind) {
        // Clear `kind`
        self.0 &= KIND_MASK;
        self.0 |= (kind as u64) << INDEX + GEN;
    }

    pub(super) fn index(&self) -> usize {
        (self.0 & INDEX_MASK) as usize
    }

    pub(crate) const fn raw(&self) -> u64 {
        self.0
    }

    pub(super) fn gen(&self) -> u16 {
        (self.0 << KIND >> KIND + INDEX) as u16
    }
}

impl From<u64> for Key {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            KeyKind::Agent => write!(f, "<A {} | {}>", self.index(), self.gen()),
            KeyKind::Writer => write!(f, "<W {} | {}>", self.index(), self.gen()),
            KeyKind::Session => write!(f, "<S {} | {}>", self.index(), self.gen()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
#[repr(u8)]
pub(crate) enum KeyKind {
    Agent = 1,
    Writer = 2,
    Session = 3,
}

#[derive(Debug)]
enum Entry<T> {
    Occupied { value: T, gen: u16 },
    Vacant(Option<Key>),
}

impl<T> Entry<T> {
    #[cfg(test)]
    fn is_occupied(&self) -> bool {
        match self {
            Entry::Occupied { .. } => true,
            Entry::Vacant(_) => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Agents<T> {
    inner: Vec<Entry<T>>,
    next_key: Option<Key>,
}

impl<T> Agents<T> {
    pub(crate) fn new() -> Self {
        Self {
            inner: vec![],
            next_key: None,
        }
    }

    pub(crate) fn insert(&mut self, value: T, kind: KeyKind) -> Key {
        match self.next_key.take() {
            None => {
                let key = Key::new(self.inner.len() as u64, kind);
                self.inner.push(Entry::Occupied {
                    value,
                    gen: key.gen(),
                });
                key
            }
            Some(mut key) => {
                let mut entry = Entry::Occupied {
                    value,
                    gen: key.gen(),
                };
                swap(&mut self.inner[key.index()], &mut entry);
                let Entry::Vacant(next) = entry else { panic!("tried to replace vacant entry") };
                self.next_key = next;
                key.set_kind(kind);
                key
            }
        }
    }

    pub(crate) fn remove(&mut self, mut key: Key) -> Option<T> {
        if key.index() >= self.inner.len() {
            return None;
        }

        let mut entry = Entry::Vacant(self.next_key.take());
        swap(&mut self.inner[key.index()], &mut entry);
        let Entry::Occupied { value, gen } = entry else { panic!() };
        if gen != key.gen() {
            panic!("generation missmatch")
        }
        key.bump();
        self.next_key = Some(key);
        Some(value)
    }

    pub(crate) fn get(&self, key: Key) -> Option<&T> {
        self.inner.get(key.index()).and_then(|entry| match entry {
            Entry::Occupied { value, gen } if *gen == key.gen() => Some(value),
            _ => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn count(&self) -> usize {
        self.inner.iter().filter(|e| e.is_occupied()).count()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn index() {
        let mut key = Key::ZERO;
        key.set_gen(2);
        key.set_index(123);

        assert_eq!(key.index(), 123);
        assert_eq!(key.gen(), 2);
    }

    #[test]
    fn insert() {
        let mut slab = Agents::new();
        let k1 = slab.insert(1, KeyKind::Agent);
        let k2 = slab.insert(2, KeyKind::Agent);

        assert_eq!(k1, Key::ZERO);
        assert_eq!(k2, Key::new(1, KeyKind::Agent));
    }

    #[test]
    fn get() {
        let mut slab = Agents::new();
        let k1 = slab.insert(1, KeyKind::Agent);

        assert_eq!(*slab.get(k1).unwrap(), 1);
    }

    #[test]
    fn remove() {
        let mut slab = Agents::new();
        let mut k1 = slab.insert(1, KeyKind::Agent);
        assert_eq!(slab.remove(k1).unwrap(), 1);
        k1.bump();
        assert_eq!(slab.next_key.unwrap(), k1);
    }

    #[test]
    fn remove_many() {
        let mut slab = Agents::new();
        let k1 = slab.insert(1, KeyKind::Agent);
        let k2 = slab.insert(2, KeyKind::Agent);
        let mut k3 = slab.insert(3, KeyKind::Agent);

        slab.remove(k1);
        slab.remove(k2);
        slab.remove(k3);

        k3.bump();
        assert_eq!(slab.next_key.unwrap(), k3);
    }

    #[test]
    fn key_kind() {
        let mut key = Key::new(0, KeyKind::Session);
        assert_eq!(key.kind(), KeyKind::Session);

        key.set_kind(KeyKind::Writer);
        assert_eq!(key.kind(), KeyKind::Writer);

        key.set_kind(KeyKind::Session);
        assert_eq!(key.kind(), KeyKind::Session);
    }

    #[test]
    fn bump_gen() {
        let mut key = Key::new(123, KeyKind::Session);
        key.set_gen(2);
        key.bump();
        assert_eq!(key.kind(), KeyKind::Session);
        assert_eq!(key.gen(), 3);
        assert_eq!(key.index(), 123);
    }
}
