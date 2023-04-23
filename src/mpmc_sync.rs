use crate::queue::StochasticQueue;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::time::Duration;

struct MpmcState<T> {
  queue: Mutex<StochasticQueue<T>>,
  condvar: Condvar,
  // If either of these become zero, it's impossible for that value to ever increase again.
  senders: AtomicUsize,
  receivers: AtomicUsize,
}

/// Sender for a stochastic MPMC channel. Create one using `stochastic_channel()`. This sender can be cheaply cloned, which will increase the sender count. Dropping a sender will decrease the sender count.
pub struct StochasticMpmcSender<T>(Arc<MpmcState<T>>);

impl<T> Clone for StochasticMpmcSender<T> {
  fn clone(&self) -> Self {
    let state = self.0.clone();
    state.senders.fetch_add(1, atomic::Ordering::Relaxed);
    Self(state)
  }
}

impl<T> Drop for StochasticMpmcSender<T> {
  fn drop(&mut self) {
    let old = self.0.senders.fetch_sub(1, atomic::Ordering::Relaxed);
    if old == 1 {
      // Any receiver blocked on recv() must now abort.
      self.0.condvar.notify_all();
    };
  }
}

/// Errors that could occur when sending an item in a stochastic channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StochasticMpmcSendError {
  NoReceivers,
}

impl Display for StochasticMpmcSendError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl Error for StochasticMpmcSendError {}

impl<T> StochasticMpmcSender<T> {
  /// Send an item. If there are no receivers, an error is returned.
  pub fn send(&self, val: T) -> Result<(), StochasticMpmcSendError> {
    if self.0.receivers.load(atomic::Ordering::Relaxed) == 0 {
      return Err(StochasticMpmcSendError::NoReceivers);
    };
    self.0.queue.lock().unwrap().push(val);
    self.0.condvar.notify_one();
    Ok(())
  }
}

/// Receiver for a stochastic MPMC channel. Create one using `stochastic_channel()`. This receiver can be cheaply cloned, which will increase the receiver count. Dropping a receiver will decrease the receiver count.
pub struct StochasticMpmcReceiver<T>(Arc<MpmcState<T>>);

impl<T> Clone for StochasticMpmcReceiver<T> {
  fn clone(&self) -> Self {
    let state = self.0.clone();
    state.receivers.fetch_add(1, atomic::Ordering::Relaxed);
    Self(state)
  }
}

impl<T> Drop for StochasticMpmcReceiver<T> {
  fn drop(&mut self) {
    self.0.receivers.fetch_sub(1, atomic::Ordering::Relaxed);
  }
}

/// Errors that could occur when receiving an item from a stochastic channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StochasticMpmcRecvError {
  NoSenders,
}

impl Display for StochasticMpmcRecvError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl Error for StochasticMpmcRecvError {}

/// Errors that could occur when receiving an item from a stochastic channel with a timeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StochasticMpmcRecvTimeoutError {
  NoSenders,
  Timeout,
}

impl Display for StochasticMpmcRecvTimeoutError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl Error for StochasticMpmcRecvTimeoutError {}

impl<T> StochasticMpmcReceiver<T> {
  /// Receive an item, or return None if no items are available right now. If there are no senders, an error is returned.
  pub fn try_recv(&self) -> Result<Option<T>, StochasticMpmcRecvError> {
    if self.0.senders.load(atomic::Ordering::Relaxed) == 0 {
      return Err(StochasticMpmcRecvError::NoSenders);
    };
    let mut queue = self.0.queue.lock().unwrap();
    Ok(queue.pop())
  }

  /// Block the current thread until an item can be received, time has run out, or there are no more senders. If time has run out or there are no more senders, an error is returned.
  pub fn recv_timeout(&self, timeout: Duration) -> Result<T, StochasticMpmcRecvTimeoutError> {
    let mut queue = self.0.queue.lock().unwrap();
    while queue.is_empty() {
      if self.0.senders.load(atomic::Ordering::Relaxed) == 0 {
        return Err(StochasticMpmcRecvTimeoutError::NoSenders);
      };
      let res = self.0.condvar.wait_timeout(queue, timeout).unwrap();
      if res.1.timed_out() {
        return Err(StochasticMpmcRecvTimeoutError::Timeout);
      };
      queue = res.0;
    }
    Ok(queue.pop().unwrap())
  }

  /// Block the current thread until an item can be received or there are no more senders. If there are no more senders, an error is returned.
  pub fn recv(&self) -> Result<T, StochasticMpmcRecvError> {
    let mut queue = self.0.queue.lock().unwrap();
    while queue.is_empty() {
      if self.0.senders.load(atomic::Ordering::Relaxed) == 0 {
        return Err(StochasticMpmcRecvError::NoSenders);
      };
      queue = self.0.condvar.wait(queue).unwrap();
    }
    Ok(queue.pop().unwrap())
  }
}

impl<T> IntoIterator for StochasticMpmcReceiver<T> {
  type IntoIter = StochasticMpmcReceiverIterator<T>;
  type Item = T;

  fn into_iter(self) -> Self::IntoIter {
    StochasticMpmcReceiverIterator { receiver: self }
  }
}

/// Iterator over items sent in a channel. This will keep producing items until there are no more senders. The current thread will be blocked while waiting for an item or until there are no more senders. Not all items sent will be received by this iterator, as there may be other receivers.
pub struct StochasticMpmcReceiverIterator<T> {
  receiver: StochasticMpmcReceiver<T>,
}

impl<T> Iterator for StochasticMpmcReceiverIterator<T> {
  type Item = T;

  fn next(&mut self) -> Option<Self::Item> {
    match self.receiver.recv() {
      Ok(v) => Some(v),
      Err(StochasticMpmcRecvError::NoSenders) => None,
    }
  }
}

/// Create a MPMC channel that receives items in a uniformly random order but without any delays. Returns a tuple containing the sender and receiver, both of which can be cheaply cloned. Senders and receivers will continue to work until there are no more senders/receivers on the other side.
///
/// # Example
///
/// ```rust
/// use std::thread;
/// use stochastic_queue::stochastic_channel;
///
/// let (sender, receiver) = stochastic_channel();
/// let r1 = receiver.clone();
/// let t1 = thread::spawn(move || {
///   for i in r1 {
///     println!("Received {i}");
///   };
/// });
/// let r2 = receiver.clone();
/// let t2 = thread::spawn(move || {
///   for i in r2 {
///     println!("Received {i}");
///   };
/// });
/// let s_even = sender.clone();
/// let t3 = thread::spawn(move || {
///   for i in (0..10).step_by(2) {
///     s_even.send(i).unwrap();
///   };
/// });
/// let s_odd = sender.clone();
/// let t4 = thread::spawn(move || {
///   for i in (1..10).step_by(2) {
///     s_odd.send(i).unwrap();
///   };
/// });
/// drop(sender);
/// drop(receiver);
/// t1.join().unwrap();
/// t2.join().unwrap();
/// t3.join().unwrap();
/// t4.join().unwrap();
/// ```
pub fn stochastic_channel<T>() -> (StochasticMpmcSender<T>, StochasticMpmcReceiver<T>) {
  let inner = Arc::new(MpmcState {
    condvar: Condvar::new(),
    queue: Mutex::new(StochasticQueue::new()),
    receivers: AtomicUsize::new(1),
    senders: AtomicUsize::new(1),
  });
  (
    StochasticMpmcSender(inner.clone()),
    StochasticMpmcReceiver(inner.clone()),
  )
}
