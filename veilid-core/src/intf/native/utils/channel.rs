pub use async_std::channel;

#[derive(Debug)]
pub struct Sender<T> {
    imp: channel::Sender<T>,
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
    imp: channel::Receiver<T>,
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            imp: self.imp.clone(),
        }
    }
}

pub fn channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let imp = channel::bounded(cap);
    (Sender { imp: imp.0 }, Receiver { imp: imp.1 })
}

pub use channel::SendError;
pub use channel::TrySendError;

#[allow(dead_code)]
impl<T> Sender<T> {
    // NOTE: This needs a timeout or you could block a very long time
    // pub async fn send(&self, msg: T) -> Result<(), SendError<T>> {
    //     self.imp.send(msg).await
    // }
    pub async fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        self.imp.try_send(msg)
    }
    pub fn capacity(&self) -> usize {
        self.imp.capacity().unwrap()
    }
    pub fn is_empty(&self) -> bool {
        self.imp.is_empty()
    }
    pub fn is_full(&self) -> bool {
        self.imp.is_full()
    }
    pub fn len(&self) -> usize {
        self.imp.len()
    }
}

pub use channel::RecvError;
pub use channel::TryRecvError;

#[allow(dead_code)]
impl<T> Receiver<T> {
    pub async fn recv(&self) -> Result<T, RecvError> {
        self.imp.recv().await
    }
    pub async fn try_recv(&self) -> Result<T, TryRecvError> {
        self.imp.try_recv()
    }
    pub fn capacity(&self) -> usize {
        self.imp.capacity().unwrap()
    }
    pub fn is_empty(&self) -> bool {
        self.imp.is_empty()
    }
    pub fn is_full(&self) -> bool {
        self.imp.is_full()
    }
    pub fn len(&self) -> usize {
        self.imp.len()
    }
}
