use std::fmt::Debug;
use std::mem::swap;

#[derive(Debug)]
enum Entry<T> {
    Occupied(T),
    Vacant(Option<u64>),
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
    next_key: Option<u64>,
}

impl<T> Slab<T> {
    pub(crate) fn new() -> Self {
        Self {
            inner: vec![],
            next_key: None,
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> u64 {
        match self.next_key.take() {
            None => {
                let key = self.inner.len();
                self.inner.push(Entry::Occupied(value));
                key as u64
            }
            Some(mut key) => {
                let mut entry = Entry::Occupied(value);
                swap(&mut self.inner[key as usize], &mut entry);
                let Entry::Vacant(next) = entry else { panic!("tried to replace vacant entry") };
                self.next_key = next;
                key
            }
        }
    }

    pub(crate) fn remove(&mut self, index: u64) -> Option<T> {
        if self.inner.len() as u64 <= index {
            return None;
        }

        let mut entry = Entry::Vacant(self.next_key.take());
        swap(&mut self.inner[index as usize], &mut entry);
        self.next_key = Some(index);
        let Entry::Occupied(value) = entry else { panic!() };
        Some(value)
    }

    pub(crate) fn get(&self, index: u64) -> Option<&T> {
        self.inner.get(index as usize).and_then(|entry| match entry {
            Entry::Occupied(value) => Some(value),
            _ => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn count(&self) -> u64 {
        self.inner.iter().filter(|e| e.is_occupied()).count() as u64
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insert() {
        let mut slab = Slab::new();
        let k1 = slab.insert(1);
        let k2 = slab.insert(2);

        assert_eq!(k1, 0);
        assert_eq!(k2, 1);
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
        assert_eq!(slab.next_key.unwrap(), k1);
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

        assert_eq!(slab.next_key.unwrap(), k3);
    }
}
