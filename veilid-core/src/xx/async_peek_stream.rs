use crate::xx::*;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_util::io::AsyncRead as Read;
use futures_util::io::AsyncReadExt;
use futures_util::io::AsyncWrite as Write;
use std::io::Result;

////////
///
trait SendStream: Read + Write + Send + Unpin {
    fn clone_stream(&self) -> Box<dyn SendStream>;
}
impl<S> SendStream for S
where
    S: Read + Write + Send + Clone + Unpin + 'static,
{
    fn clone_stream(&self) -> Box<dyn SendStream> {
        Box::new(self.clone())
    }
}

////////
///

pub struct Peek<'a> {
    aps: AsyncPeekStream,
    buf: &'a mut [u8],
}

impl Unpin for Peek<'_> {}

impl Future for Peek<'_> {
    type Output = std::io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;

        let mut inner = this.aps.inner.lock();
        let inner = &mut *inner;
        //
        let buf_len = this.buf.len();
        let mut copy_len = buf_len;
        if buf_len > inner.peekbuf_len {
            //
            inner.peekbuf.resize(buf_len, 0u8);
            let mut read_future = inner
                .stream
                .read(&mut inner.peekbuf.as_mut_slice()[inner.peekbuf_len..buf_len]);
            let read_len = match Pin::new(&mut read_future).poll(cx) {
                Poll::Pending => {
                    inner.peekbuf.resize(inner.peekbuf_len, 0u8);
                    return Poll::Pending;
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(Ok(v)) => v,
            };
            inner.peekbuf_len += read_len;
            inner.peekbuf.resize(inner.peekbuf_len, 0u8);
            copy_len = inner.peekbuf_len;
        }
        this.buf[..copy_len].copy_from_slice(&inner.peekbuf[..copy_len]);
        Poll::Ready(Ok(copy_len))
    }
}

////////
///

pub struct PeekExact<'a> {
    aps: AsyncPeekStream,
    buf: &'a mut [u8],
}

impl Unpin for PeekExact<'_> {}

impl Future for PeekExact<'_> {
    type Output = std::io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;

        let mut inner = this.aps.inner.lock();
        let inner = &mut *inner;
        //
        let buf_len = this.buf.len();
        let mut copy_len = buf_len;
        if buf_len > inner.peekbuf_len {
            //
            inner.peekbuf.resize(buf_len, 0u8);
            let mut read_future = inner
                .stream
                .read_exact(&mut inner.peekbuf.as_mut_slice()[inner.peekbuf_len..buf_len]);
            match Pin::new(&mut read_future).poll(cx) {
                Poll::Pending => {
                    inner.peekbuf.resize(inner.peekbuf_len, 0u8);
                    return Poll::Pending;
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(Ok(())) => (),
            };
            inner.peekbuf_len = buf_len;
            copy_len = inner.peekbuf_len;
        }
        this.buf[..copy_len].copy_from_slice(&inner.peekbuf[..copy_len]);
        Poll::Ready(Ok(copy_len))
    }
}
/////////
///
struct AsyncPeekStreamInner {
    stream: Box<dyn SendStream>,
    peekbuf: Vec<u8>,
    peekbuf_len: usize,
}

#[derive(Clone)]
pub struct AsyncPeekStream
where
    Self: Read + Write + Send + Unpin,
{
    inner: Arc<Mutex<AsyncPeekStreamInner>>,
}

impl AsyncPeekStream {
    pub fn new<S>(stream: S) -> Self
    where
        S: Read + Write + Send + Clone + Unpin + 'static,
    {
        Self {
            inner: Arc::new(Mutex::new(AsyncPeekStreamInner {
                stream: Box::new(stream),
                peekbuf: Vec::new(),
                peekbuf_len: 0,
            })),
        }
    }

    pub fn peek<'a>(&'a self, buf: &'a mut [u8]) -> Peek<'a> {
        Peek::<'a> {
            aps: self.clone(),
            buf,
        }
    }

    pub fn peek_exact<'a>(&'a self, buf: &'a mut [u8]) -> PeekExact<'a> {
        PeekExact::<'a> {
            aps: self.clone(),
            buf,
        }
    }
}

impl Read for AsyncPeekStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let mut inner = self.inner.lock();
        //
        let buflen = buf.len();
        let bufcopylen = cmp::min(buflen, inner.peekbuf_len);
        let bufreadlen = if buflen > inner.peekbuf_len {
            buflen - inner.peekbuf_len
        } else {
            0
        };

        if bufreadlen > 0 {
            match Pin::new(&mut inner.stream).poll_read(cx, &mut buf[bufcopylen..buflen]) {
                Poll::Ready(res) => {
                    let readlen = res?;
                    buf[0..bufcopylen].copy_from_slice(&inner.peekbuf[0..bufcopylen]);
                    inner.peekbuf_len = 0;
                    Poll::Ready(Ok(bufcopylen + readlen))
                }
                Poll::Pending => {
                    if bufcopylen > 0 {
                        buf[0..bufcopylen].copy_from_slice(&inner.peekbuf[0..bufcopylen]);
                        inner.peekbuf_len = 0;
                        Poll::Ready(Ok(bufcopylen))
                    } else {
                        Poll::Pending
                    }
                }
            }
        } else {
            buf[0..bufcopylen].copy_from_slice(&inner.peekbuf[0..bufcopylen]);
            if bufcopylen == inner.peekbuf_len {
                inner.peekbuf_len = 0;
            } else if bufcopylen != 0 {
                // slide buffer over by bufcopylen
                let tail = inner.peekbuf.split_off(bufcopylen);
                inner.peekbuf = tail;
                inner.peekbuf_len -= bufcopylen;
            }
            Poll::Ready(Ok(bufcopylen))
        }
    }
}

impl Write for AsyncPeekStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut inner.stream).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut inner.stream).poll_flush(cx)
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut inner = self.inner.lock();
        Pin::new(&mut inner.stream).poll_close(cx)
    }
}

impl core::marker::Unpin for AsyncPeekStream {}
