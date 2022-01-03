use crate::xx::*;
use async_std::io::{Read, ReadExt, Result, Write};
use core::pin::Pin;
use core::task::{Context, Poll};

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

    pub async fn peek(&'_ self, buf: &'_ mut [u8]) -> Result<usize> {
        let (mut stream, mut peekbuf, mut peekbuf_len) = {
            let inner = self.inner.lock();
            (
                inner.stream.clone_stream(),
                inner.peekbuf.clone(),
                inner.peekbuf_len,
            )
        };
        //
        let buf_len = buf.len();
        let mut copy_len = buf_len;
        if buf_len > peekbuf_len {
            //
            peekbuf.resize(buf_len, 0u8);
            let read_len = stream
                .read(&mut peekbuf.as_mut_slice()[peekbuf_len..buf_len])
                .await?;
            peekbuf_len += read_len;
            copy_len = peekbuf_len;
        }
        buf[..copy_len].copy_from_slice(&peekbuf[..copy_len]);

        let mut inner = self.inner.lock();
        inner.peekbuf = peekbuf;
        inner.peekbuf_len = peekbuf_len;
        Ok(copy_len)
    }

    pub async fn peek_exact(&'_ self, buf: &'_ mut [u8]) -> Result<()> {
        let (mut stream, mut peekbuf, mut peekbuf_len) = {
            let inner = self.inner.lock();
            (
                inner.stream.clone_stream(),
                inner.peekbuf.clone(),
                inner.peekbuf_len,
            )
        };
        //
        let buf_len = buf.len();
        if buf_len > peekbuf_len {
            //
            peekbuf.resize(buf_len, 0u8);
            stream
                .read_exact(&mut peekbuf.as_mut_slice()[peekbuf_len..buf_len])
                .await?;
            peekbuf_len = buf_len;
        }
        buf.copy_from_slice(&peekbuf[..buf_len]);

        let mut inner = self.inner.lock();
        inner.peekbuf = peekbuf;
        inner.peekbuf_len = peekbuf_len;
        Ok(())
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

impl std::marker::Unpin for AsyncPeekStream {}
