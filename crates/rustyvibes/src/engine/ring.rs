//! Wait-free single-producer / single-consumer ring buffer for `Copy` items.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Keeps the producer and consumer indices on separate cache lines.
#[repr(align(128))]
struct Padded(AtomicUsize);

struct Inner<T> {
    /// Next slot to read; written only by the consumer.
    head: Padded,
    /// Next slot to write; written only by the producer.
    tail: Padded,
    mask: usize,
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

// SAFETY: each slot is written by the single producer before it publishes `tail`
// (Release) and read by the single consumer after it observes `tail` (Acquire), and
// vice versa for `head`, so no slot is ever accessed concurrently.
unsafe impl<T: Send> Sync for Inner<T> {}

pub struct Producer<T> {
    inner: Arc<Inner<T>>,
    cached_head: usize,
}

pub struct Consumer<T> {
    inner: Arc<Inner<T>>,
    cached_tail: usize,
}

/// Creates a ring holding at least `capacity` items (rounded up to a power of two).
pub fn ring<T: Copy + Send>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let capacity = capacity.next_power_of_two().max(2);
    let slots = (0..capacity).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect();
    let inner = Arc::new(Inner {
        head: Padded(AtomicUsize::new(0)),
        tail: Padded(AtomicUsize::new(0)),
        mask: capacity - 1,
        slots,
    });
    (Producer { inner: inner.clone(), cached_head: 0 }, Consumer { inner, cached_tail: 0 })
}

impl<T: Copy + Send> Producer<T> {
    /// Appends `item`; returns `false` (dropping it) when the ring is full.
    pub fn push(&mut self, item: T) -> bool {
        let tail = self.inner.tail.0.load(Ordering::Relaxed);
        if tail.wrapping_sub(self.cached_head) > self.inner.mask {
            self.cached_head = self.inner.head.0.load(Ordering::Acquire);
            if tail.wrapping_sub(self.cached_head) > self.inner.mask {
                return false;
            }
        }
        // SAFETY: the slot is free (checked above) and invisible to the consumer until
        // `tail` is published below.
        unsafe { (*self.inner.slots[tail & self.inner.mask].get()).write(item) };
        self.inner.tail.0.store(tail.wrapping_add(1), Ordering::Release);
        true
    }
}

impl<T: Copy + Send> Consumer<T> {
    /// Removes the oldest item.
    pub fn pop(&mut self) -> Option<T> {
        let head = self.inner.head.0.load(Ordering::Relaxed);
        if head == self.cached_tail {
            self.cached_tail = self.inner.tail.0.load(Ordering::Acquire);
            if head == self.cached_tail {
                return None;
            }
        }
        // SAFETY: the producer initialised this slot before publishing `tail` (Release),
        // which we observed with Acquire.
        let item = unsafe { (*self.inner.slots[head & self.inner.mask].get()).assume_init_read() };
        self.inner.head.0.store(head.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.head.0.load(Ordering::Relaxed) == self.inner.tail.0.load(Ordering::Acquire)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_order_and_capacity() {
        let (mut tx, mut rx) = ring::<u32>(4);
        for i in 0..4 {
            assert!(tx.push(i));
        }
        assert!(!tx.push(99), "full");
        for i in 0..4 {
            assert_eq!(rx.pop(), Some(i));
        }
        assert_eq!(rx.pop(), None);
        assert!(rx.is_empty());
    }

    #[test]
    fn capacity_rounds_up_to_a_power_of_two() {
        let (mut tx, _rx) = ring::<u8>(5);
        assert_eq!((0..100).filter(|_| tx.push(1)).count(), 8);
    }

    #[test]
    fn wraps_around_many_times() {
        let (mut tx, mut rx) = ring::<u64>(8);
        for i in 0..10_000u64 {
            assert!(tx.push(i));
            assert!(!rx.is_empty());
            assert_eq!(rx.pop(), Some(i));
        }
    }

    #[test]
    fn concurrent_stress_preserves_order() {
        const N: u64 = 1_000_000;
        let (mut tx, mut rx) = ring::<u64>(64);
        let producer = std::thread::spawn(move || {
            for i in 0..N {
                while !tx.push(i) {
                    std::hint::spin_loop();
                }
            }
        });
        let mut expected = 0;
        while expected < N {
            if let Some(v) = rx.pop() {
                assert_eq!(v, expected);
                expected += 1;
            } else {
                std::hint::spin_loop();
            }
        }
        producer.join().unwrap();
    }
}
