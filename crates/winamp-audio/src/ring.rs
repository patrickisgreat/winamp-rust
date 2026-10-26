//! Lock-free single-producer / single-consumer ring buffer of `f32` samples.
//!
//! The decode thread is the producer, the audio device callback is the consumer.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Inner {
    /// Every slot is its own `UnsafeCell` so the producer and consumer never
    /// form overlapping references to the whole buffer.
    slots: Box<[UnsafeCell<f32>]>,
    mask: usize,
    /// Read position, advanced only by the consumer.
    head: AtomicUsize,
    /// Write position, advanced only by the producer.
    tail: AtomicUsize,
}

// Safety: the producer only writes slots in [tail, head + capacity) and then
// publishes `tail` with Release; the consumer only reads slots in [head, tail)
// after loading `tail` with Acquire, and vice versa for `head`. A slot is
// therefore never accessed by both sides at the same time.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl Inner {
    fn capacity(&self) -> usize {
        self.mask + 1
    }

    fn buffered(&self, head: usize, tail: usize) -> usize {
        tail.wrapping_sub(head)
    }
}

/// The writing half of a ring buffer.
pub struct Producer(Arc<Inner>);
/// The reading half of a ring buffer.
pub struct Consumer(Arc<Inner>);

/// Create a ring buffer holding `capacity` samples (rounded up to a power of two).
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let capacity = capacity.next_power_of_two();
    let inner = Arc::new(Inner {
        slots: (0..capacity).map(|_| UnsafeCell::new(0.0)).collect(),
        mask: capacity - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    (Producer(Arc::clone(&inner)), Consumer(inner))
}

impl Producer {
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Number of samples currently buffered.
    pub fn len(&self) -> usize {
        let head = self.0.head.load(Ordering::Acquire);
        let tail = self.0.tail.load(Ordering::Relaxed);
        self.0.buffered(head, tail)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of samples that can be pushed right now.
    pub fn free(&self) -> usize {
        self.capacity() - self.len()
    }

    /// Push as many samples as fit; returns how many were written.
    pub fn push(&mut self, data: &[f32]) -> usize {
        let count = data.len().min(self.free());
        let tail = self.0.tail.load(Ordering::Relaxed);
        for (offset, &sample) in data[..count].iter().enumerate() {
            let slot = &self.0.slots[(tail + offset) & self.0.mask];
            // Safety: see the `Inner` impl comment; these slots are free.
            unsafe { *slot.get() = sample };
        }
        self.0
            .tail
            .store(tail.wrapping_add(count), Ordering::Release);
        count
    }
}

impl Consumer {
    /// Number of samples currently buffered.
    pub fn len(&self) -> usize {
        let tail = self.0.tail.load(Ordering::Acquire);
        let head = self.0.head.load(Ordering::Relaxed);
        self.0.buffered(head, tail)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pop up to `out.len()` samples; returns how many were read. Unfilled
    /// entries are left untouched.
    pub fn pop(&mut self, out: &mut [f32]) -> usize {
        let count = out.len().min(self.len());
        let head = self.0.head.load(Ordering::Relaxed);
        for (offset, target) in out[..count].iter_mut().enumerate() {
            let slot = &self.0.slots[(head + offset) & self.0.mask];
            // Safety: see the `Inner` impl comment; these slots are published.
            *target = unsafe { *slot.get() };
        }
        self.0
            .head
            .store(head.wrapping_add(count), Ordering::Release);
        count
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
        let (mut producer, mut consumer) = ring(8);
        assert!(producer.is_empty());
        assert_eq!(producer.push(&[1.0, 2.0, 3.0]), 3);
        let mut out = [0.0; 2];
        assert_eq!(consumer.pop(&mut out), 2);
        assert_eq!(out, [1.0, 2.0]);
        assert_eq!(producer.push(&[4.0; 10]), 7);
        assert_eq!(consumer.len(), 8);
        assert_eq!(producer.free(), 0);
        consumer.clear();
        assert!(consumer.is_empty());
    }

    #[test]
    fn capacity_rounds_up_to_power_of_two() {
        let (producer, _) = ring(5);
        assert_eq!(producer.capacity(), 8);
    }

    #[test]
    fn preserves_order_across_wraparound() {
        let (mut producer, mut consumer) = ring(4);
        let mut expected = 0.0;
        let mut scratch = [0.0; 3];
        for _ in 0..50 {
            let batch: Vec<f32> = (0..3).map(|i| expected + i as f32).collect();
            assert_eq!(producer.push(&batch), 3);
            assert_eq!(consumer.pop(&mut scratch), 3);
            assert_eq!(scratch, batch.as_slice());
            expected += 3.0;
        }
    }

    #[test]
    fn pop_on_empty_leaves_output_untouched() {
        let (_, mut consumer) = ring(4);
        let mut out = [7.0; 2];
        assert_eq!(consumer.pop(&mut out), 0);
        assert_eq!(out, [7.0, 7.0]);
    }

    #[test]
    fn transfers_every_sample_between_threads_in_order() {
        const TOTAL: usize = 100_000;
        let (mut producer, mut consumer) = ring(256);
        let writer = std::thread::spawn(move || {
            let mut next = 0usize;
            while next < TOTAL {
                let batch: Vec<f32> = (next..(next + 64).min(TOTAL)).map(|i| i as f32).collect();
                let written = producer.push(&batch);
                next += written;
                if written == 0 {
                    std::thread::yield_now();
                }
            }
        });
        let mut received = Vec::with_capacity(TOTAL);
        let mut scratch = [0.0; 100];
        while received.len() < TOTAL {
            let got = consumer.pop(&mut scratch);
            received.extend_from_slice(&scratch[..got]);
            if got == 0 {
                std::thread::yield_now();
            }
        }
        writer.join().expect("writer thread panicked");
        assert!(received.iter().enumerate().all(|(i, &v)| v == i as f32));
    }
}
