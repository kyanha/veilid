use crate::xx::*;
use alloc::collections::VecDeque;
use core::fmt;

#[derive(Debug)]
pub struct Channel<T> {
    items: VecDeque<T>,
    cap: usize,
    eventual: Eventual,
}

#[derive(Debug)]
pub struct Sender<T> {
    imp: Arc<Mutex<Channel<T>>>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            imp: self.imp.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Receiver<T> {
    imp: Arc<Mutex<Channel<T>>>,
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            imp: self.imp.clone(),
        }
    }
}

pub fn channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let imp = Channel {
        items: VecDeque::with_capacity(cap),
        cap,
        eventual: Eventual::new(),
    };

    let imparc = Arc::new(Mutex::new(imp));
    (
        Sender {
            imp: imparc.clone(),
        },
        Receiver {
            imp: imparc.clone(),
        },
    )
}

#[derive(Debug, PartialEq, Eq)]
pub enum TrySendError<T> {
    Full(T),
    Disconnected(T),
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => {
                write!(f, "Full")
            }
            TrySendError::Disconnected(_) => {
                write!(f, "Disconnected")
            }
        }
    }
}

impl<T> Sender<T> {
    // NOTE: This needs a timeout or you could block a very long time
    // pub async fn send(&self, msg: T) -> Result<(), SendError<T>> {
    //     xxx
    // }

    pub async fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        let eventual = {
            let mut inner = self.imp.lock();
            if inner.items.len() == inner.cap {
                return Err(TrySendError::Full(msg));
            }
            let empty = inner.items.is_empty();
            inner.items.push_back(msg);
            if empty {
                Some(inner.eventual.clone())
            } else {
                None
            }
        };
        if let Some(e) = eventual {
            e.resolve().await;
        }
        Ok(())
    }
    pub fn capacity(&self) -> usize {
        self.imp.lock().cap
    }
    pub fn is_empty(&self) -> bool {
        self.imp.lock().items.is_empty()
    }
    pub fn is_full(&self) -> bool {
        let inner = self.imp.lock();
        inner.items.len() == inner.cap
    }
    pub fn len(&self) -> usize {
        self.imp.lock().items.len()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RecvError;

#[derive(Debug, PartialEq, Eq)]
pub enum TryRecvError {
    Empty,
    Disconnected,
}

impl<T> Receiver<T> {
    pub async fn recv(&self) -> Result<T, RecvError> {
        let eventual = {
            let inner = self.imp.lock();
            inner.eventual.clone()
        };
        while self.is_empty() {
            eventual.instance_clone(true).await;
        }
        Ok(self.imp.lock().items.pop_front().unwrap())
    }
    pub async fn try_recv(&self) -> Result<T, TryRecvError> {
        if self.is_empty() {
            return Err(TryRecvError::Empty);
        }
        Ok(self.imp.lock().items.pop_front().unwrap())
    }
    pub fn capacity(&self) -> usize {
        self.imp.lock().cap
    }
    pub fn is_empty(&self) -> bool {
        self.imp.lock().items.is_empty()
    }
    pub fn is_full(&self) -> bool {
        let inner = self.imp.lock();
        inner.items.len() == inner.cap
    }
    pub fn len(&self) -> usize {
        self.imp.lock().items.len()
    }
}
