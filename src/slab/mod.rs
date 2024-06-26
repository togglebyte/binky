use std::fmt::Debug;
use std::mem::swap;

// pub(crate) use self::keys::{Key, KeyKind, RemoteKey};

// mod keys;

#[derive(Debug)]
enum Entry<T> {
    Occupied { value: T },
    Vacant(Option<usize>),
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
pub(crate) struct Slab<T> {
    inner: Vec<Entry<T>>,
    next_key: Option<Key>,
}

impl<T> Slab<T> {
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
                key.kind = kind;
                key
            }
        }
    }

    pub(crate) fn remove(&mut self, key: Key) -> Option<T> {
        if key.index() >= self.inner.len() {
            return None;
        }

        let mut entry = Entry::Vacant(self.next_key.take());
        swap(&mut self.inner[key.index()], &mut entry);
        self.next_key = Some(key.bump());
        let Entry::Occupied { value, gen } = entry else { panic!() };
        if gen != key.gen() {
            panic!()
        }
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
        key.set_gen(u16::MAX);
        key.set_index(123);

        assert_eq!(key.index(), 123);
        assert_eq!(key.gen(), u16::MAX);
    }

    #[test]
    fn insert() {
        let mut slab = Slab::new();
        let k1 = slab.insert(1);
        let k2 = slab.insert(2);

        assert_eq!(k1, Key::ZERO);
        assert_eq!(k2, BaseKey::new(1));
    }

    #[test]
    fn get() {
        let mut slab = Slab::new();
        let k1 = slab.insert(1);

        assert_eq!(*slab.get(k1).unwrap(), 1);
    }

    #[test]
    fn remove() {
        let mut slab = Slab::new();
        let k1 = slab.insert(1);
        assert_eq!(slab.remove(k1).unwrap(), 1);
        assert_eq!(slab.next_key.unwrap(), k1.bump());
    }

    #[test]
    fn remove_many() {
        let mut slab = Slab::new();
        let k1 = slab.insert(1);
        let k2 = slab.insert(2);
        let k3 = slab.insert(3);

        slab.remove(k1);
        slab.remove(k2);
        slab.remove(k3);

        assert_eq!(slab.next_key.unwrap(), k3.bump());
    }
}
