use std::collections::VecDeque;

#[derive(Debug)]
pub(crate) struct Queue<T> {
    cap: usize,
    inner: VecDeque<T>,
}

impl<T> Queue<T> {
    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(cap),
            cap,
        }
    }

    pub(crate) fn push(&mut self, value: T) {
        if self.inner.len() + 1 == self.inner.capacity() {
            let _ = self.inner.pop_front();
        }
        self.inner.push_back(value);
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        self.inner.pop_front()
    }
}
