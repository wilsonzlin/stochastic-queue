use rand::thread_rng;
use rand::RngCore;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

struct Entry<T> {
  pos: u64,
  val: T,
}

impl<T> PartialEq for Entry<T> {
  fn eq(&self, other: &Self) -> bool {
    self.pos == other.pos
  }
}

impl<T> Eq for Entry<T> {}

impl<T> PartialOrd for Entry<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.pos.partial_cmp(&other.pos)
  }
}

impl<T> Ord for Entry<T> {
  fn cmp(&self, other: &Self) -> Ordering {
    self.partial_cmp(other).unwrap()
  }
}

/// A queue which pops items at random uniformly. It cannot be iterated or peeked.
pub struct StochasticQueue<T> {
  entries: BinaryHeap<Entry<T>>,
}

impl<T> StochasticQueue<T> {
  /// Create a queue with an initial capacity.
  pub fn with_capacity(cap: usize) -> Self {
    Self {
      entries: BinaryHeap::with_capacity(cap),
    }
  }

  /// Create a queue.
  pub fn new() -> Self {
    Self::with_capacity(0)
  }

  /// Get the amount of items in the queue. These items can be popped.
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Checks whether or not the queue has no items.
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Adds an item to the queue. When it will be chosen to be popped is nondeterministic.
  pub fn push(&mut self, val: T) {
    let pos = thread_rng().next_u64();
    self.entries.push(Entry { pos, val });
  }

  /// Removes an item from the queue, or returns None if the queue is empty. Which item will be popped is nondeterministic, but all items have an equal chance of being popped. If there is at least one item present, an item will definitely be popped.
  pub fn pop(&mut self) -> Option<T> {
    self.entries.pop().map(|e| e.val)
  }

  /// Removes all items from the queue.
  pub fn clear(&mut self) {
    self.entries.clear();
  }
}
