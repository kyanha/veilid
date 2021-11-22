use crate::xx::*;
use async_std::io::{Read, Result, Write};
use core::task::{Context, Poll};
use std::pin::Pin;

pub struct CloneStream<T>
where
    T: Read + Write + Send + Unpin,
{
    inner: Arc<Mutex<T>>,
}

impl<T> Clone for CloneStream<T>
where
    T: Read + Write + Send + Unpin,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> CloneStream<T>
where
    T: Read + Write + Send + Unpin,
{
    pub fn new(t: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(t)),
        }
    }
}
impl<T> Read for CloneStream<T>
where
    T: Read + Write + Send + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut *inner).poll_read(cx, buf)
    }
}

impl<T> Write for CloneStream<T>
where
    T: Read + Write + Send + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut *inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut *inner).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut *inner).poll_close(cx)
    }
}
