//! Lock-free single-producer / single-consumer ring buffer of `f32` samples.
//!
//! The decode thread is the producer, the audio device callback is the consumer.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Inner {
    buf: UnsafeCell<Vec<f32>>,
    mask: usize,
    head: AtomicUsize, // read position (consumer)
    tail: AtomicUsize, // write position (producer)
}

// Safety: the producer only touches `tail` and writes to slots in [tail, head+cap),
// the consumer only touches `head` and reads slots in [head, tail). The atomics
// provide the required happens-before edges.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

pub struct Producer(Arc<Inner>);
pub struct Consumer(Arc<Inner>);

/// Create a ring buffer holding `capacity` samples (rounded up to a power of two).
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let cap = capacity.next_power_of_two();
    let inner = Arc::new(Inner {
        buf: UnsafeCell::new(vec![0.0; cap]),
        mask: cap - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    (Producer(inner.clone()), Consumer(inner))
}

impl Producer {
    pub fn capacity(&self) -> usize {
        self.0.mask + 1
    }

    pub fn len(&self) -> usize {
        let head = self.0.head.load(Ordering::Acquire);
        let tail = self.0.tail.load(Ordering::Relaxed);
        tail.wrapping_sub(head)
    }

    pub fn free(&self) -> usize {
        self.capacity() - self.len()
    }

    /// Push as many samples as fit; returns how many were written.
    pub fn push(&mut self, data: &[f32]) -> usize {
        let free = self.free();
        let n = data.len().min(free);
        let tail = self.0.tail.load(Ordering::Relaxed);
        let buf = unsafe { &mut *self.0.buf.get() };
        for (i, s) in data[..n].iter().enumerate() {
            buf[(tail + i) & self.0.mask] = *s;
        }
        self.0.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }
}

impl Consumer {
    pub fn len(&self) -> usize {
        let tail = self.0.tail.load(Ordering::Acquire);
        let head = self.0.head.load(Ordering::Relaxed);
        tail.wrapping_sub(head)
    }

    /// Pop up to `out.len()` samples; returns how many were read. Unfilled
    /// entries are left untouched.
    pub fn pop(&mut self, out: &mut [f32]) -> usize {
        let avail = self.len();
        let n = out.len().min(avail);
        let head = self.0.head.load(Ordering::Relaxed);
        let buf = unsafe { &*self.0.buf.get() };
        for (i, o) in out[..n].iter_mut().enumerate() {
            *o = buf[(head + i) & self.0.mask];
        }
        self.0.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    /// Discard everything currently buffered.
    pub fn clear(&mut self) {
        let tail = self.0.tail.load(Ordering::Acquire);
        self.0.head.store(tail, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let (mut p, mut c) = ring(8);
        assert_eq!(p.push(&[1.0, 2.0, 3.0]), 3);
        let mut out = [0.0; 2];
        assert_eq!(c.pop(&mut out), 2);
        assert_eq!(out, [1.0, 2.0]);
        assert_eq!(p.push(&[4.0; 10]), 7);
        assert_eq!(c.len(), 8);
        c.clear();
        assert_eq!(c.len(), 0);
    }
}
