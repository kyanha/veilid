use async_std::channel::{bounded, Receiver, RecvError, Sender, TrySendError};
use std::sync::Arc;

#[derive(Debug)]
struct ClientLogChannelInner {
    sender: Sender<String>,
    receiver: Receiver<String>,
}

#[derive(Debug, Clone)]
pub struct ClientLogChannel {
    inner: Arc<ClientLogChannelInner>,
}

impl ClientLogChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(1);
        Self {
            inner: Arc::new(ClientLogChannelInner { sender, receiver }),
        }
    }

    pub async fn recv(&self) -> Result<String, RecvError> {
        self.inner.receiver.recv().await
    }
}

impl std::io::Write for ClientLogChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Err(e) = self
            .inner
            .sender
            .try_send(String::from_utf8_lossy(buf).to_string())
        {
            match e {
                TrySendError::Full(_) => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
                TrySendError::Closed(_) => {
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                }
            }
        } else {
            Ok(buf.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
