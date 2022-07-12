use super::*;
use futures_util::{AsyncReadExt, AsyncWriteExt};
use sockets::*;

pub struct RawTcpNetworkConnection {
    descriptor: ConnectionDescriptor,
    stream: AsyncPeekStream,
}

impl fmt::Debug for RawTcpNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawTCPNetworkConnection").finish()
    }
}

impl RawTcpNetworkConnection {
    pub fn new(descriptor: ConnectionDescriptor, stream: AsyncPeekStream) -> Self {
        Self { descriptor, stream }
    }

    pub fn descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }

    // #[instrument(level = "trace", err, skip(self))]
    // pub async fn close(&mut self) -> io::Result<()> {
    //     // Make an attempt to flush the stream
    //     self.stream.clone().close().await?;
    //     // Then shut down the write side of the socket to effect a clean close
    //     cfg_if! {
    //         if #[cfg(feature="rt-async-std")] {
    //             self.tcp_stream
    //                 .shutdown(async_std::net::Shutdown::Write)
    //         } else if #[cfg(feature="rt-tokio")] {
    //             use tokio::io::AsyncWriteExt;
    //             self.tcp_stream.get_mut()
    //                 .shutdown()
    //                 .await
    //         }
    //     }
    // }

    async fn send_internal(stream: &mut AsyncPeekStream, message: Vec<u8>) -> io::Result<()> {
        log_net!("sending TCP message of size {}", message.len());
        if message.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large TCP message");
        }
        let len = message.len() as u16;
        let header = [b'V', b'L', len as u8, (len >> 8) as u8];

        stream.write_all(&header).await?;
        stream.write_all(&message).await
    }

    #[instrument(level="trace", err, skip(self, message), fields(message.len = message.len()))]
    pub async fn send(&self, message: Vec<u8>) -> io::Result<()> {
        let mut stream = self.stream.clone();
        Self::send_internal(&mut stream, message).await
    }

    pub async fn recv_internal(stream: &mut AsyncPeekStream) -> io::Result<Vec<u8>> {
        let mut header = [0u8; 4];

        stream.read_exact(&mut header).await?;

        if header[0] != b'V' || header[1] != b'L' {
            bail_io_error_other!("received invalid TCP frame header");
        }
        let len = ((header[3] as usize) << 8) | (header[2] as usize);
        if len > MAX_MESSAGE_SIZE {
            bail_io_error_other!("received too large TCP frame");
        }

        let mut out: Vec<u8> = vec![0u8; len];
        stream.read_exact(&mut out).await?;

        Ok(out)
    }

    #[instrument(level="trace", err, skip(self), fields(ret.len))]
    pub async fn recv(&self) -> io::Result<Vec<u8>> {
        let mut stream = self.stream.clone();
        let out = Self::recv_internal(&mut stream).await?;
        tracing::Span::current().record("ret.len", &out.len());
        Ok(out)
    }
}

///////////////////////////////////////////////////////////
///

struct RawTcpProtocolHandlerInner {
    local_address: SocketAddr,
}

#[derive(Clone)]
pub struct RawTcpProtocolHandler
where
    Self: ProtocolAcceptHandler,
{
    inner: Arc<Mutex<RawTcpProtocolHandlerInner>>,
}

impl RawTcpProtocolHandler {
    fn new_inner(local_address: SocketAddr) -> RawTcpProtocolHandlerInner {
        RawTcpProtocolHandlerInner { local_address }
    }

    pub fn new(local_address: SocketAddr) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(local_address))),
        }
    }

    #[instrument(level = "trace", err, skip(self, stream))]
    async fn on_accept_async(
        self,
        stream: AsyncPeekStream,
        socket_addr: SocketAddr,
    ) -> io::Result<Option<ProtocolNetworkConnection>> {
        log_net!("TCP: on_accept_async: enter");
        let mut peekbuf: [u8; PEEK_DETECT_LEN] = [0u8; PEEK_DETECT_LEN];
        let peeklen = stream.peek(&mut peekbuf).await?;
        assert_eq!(peeklen, PEEK_DETECT_LEN);

        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(socket_addr),
            ProtocolType::TCP,
        );
        let local_address = self.inner.lock().local_address;
        let conn = ProtocolNetworkConnection::RawTcp(RawTcpNetworkConnection::new(
            ConnectionDescriptor::new(peer_addr, SocketAddress::from_socket_addr(local_address)),
            stream,
        ));

        log_net!(debug "TCP: on_accept_async from: {}", socket_addr);

        Ok(Some(conn))
    }

    #[instrument(level = "trace", err)]
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> io::Result<ProtocolNetworkConnection> {
        // Get remote socket address to connect to
        let remote_socket_addr = dial_info.to_socket_addr();

        // Make a shared socket
        let socket = match local_address {
            Some(a) => new_bound_shared_tcp_socket(a)?,
            None => {
                new_unbound_shared_tcp_socket(socket2::Domain::for_address(remote_socket_addr))?
            }
        };

        // Non-blocking connect to remote address
        let ts = nonblocking_connect(socket, remote_socket_addr).await?;

        // See what local address we ended up with and turn this into a stream
        let actual_local_address = ts.local_addr()?;
        #[cfg(feature = "rt-tokio")]
        let ts = ts.compat();
        let ps = AsyncPeekStream::new(ts);

        // Wrap the stream in a network connection and return it
        let conn = ProtocolNetworkConnection::RawTcp(RawTcpNetworkConnection::new(
            ConnectionDescriptor::new(
                dial_info.to_peer_address(),
                SocketAddress::from_socket_addr(actual_local_address),
            ),
            ps,
        ));

        Ok(conn)
    }

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len()))]
    pub async fn send_unbound_message(socket_addr: SocketAddr, data: Vec<u8>) -> io::Result<()> {
        if data.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large unbound TCP message");
        }
        trace!(
            "sending unbound message of length {} to {}",
            data.len(),
            socket_addr
        );

        // Make a shared socket
        let socket = new_unbound_shared_tcp_socket(socket2::Domain::for_address(socket_addr))?;

        // Non-blocking connect to remote address
        let ts = nonblocking_connect(socket, socket_addr).await?;

        // See what local address we ended up with and turn this into a stream
        // let actual_local_address = ts
        //     .local_addr()
        //     .map_err(map_to_string)
        //     .map_err(logthru_net!("could not get local address from TCP stream"))?;

        #[cfg(feature = "rt-tokio")]
        let ts = ts.compat();
        let mut ps = AsyncPeekStream::new(ts);

        // Send directly from the raw network connection
        // this builds the connection and tears it down immediately after the send
        RawTcpNetworkConnection::send_internal(&mut ps, data).await
    }

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len(), ret.len))]
    pub async fn send_recv_unbound_message(
        socket_addr: SocketAddr,
        data: Vec<u8>,
        timeout_ms: u32,
    ) -> io::Result<Vec<u8>> {
        if data.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large unbound TCP message");
        }
        trace!(
            "sending unbound message of length {} to {}",
            data.len(),
            socket_addr
        );

        // Make a shared socket
        let socket = new_unbound_shared_tcp_socket(socket2::Domain::for_address(socket_addr))?;

        // Non-blocking connect to remote address
        let ts = nonblocking_connect(socket, socket_addr).await?;

        // See what local address we ended up with and turn this into a stream
        // let actual_local_address = ts
        //     .local_addr()
        //     .map_err(map_to_string)
        //     .map_err(logthru_net!("could not get local address from TCP stream"))?;
        #[cfg(feature = "rt-tokio")]
        let ts = ts.compat();
        let mut ps = AsyncPeekStream::new(ts);

        // Send directly from the raw network connection
        // this builds the connection and tears it down immediately after the send
        RawTcpNetworkConnection::send_internal(&mut ps, data).await?;

        let out = timeout(timeout_ms, RawTcpNetworkConnection::recv_internal(&mut ps))
            .await
            .map_err(|e| e.to_io())??;

        tracing::Span::current().record("ret.len", &out.len());
        Ok(out)
    }
}

impl ProtocolAcceptHandler for RawTcpProtocolHandler {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
    ) -> SystemPinBoxFuture<io::Result<Option<ProtocolNetworkConnection>>> {
        Box::pin(self.clone().on_accept_async(stream, peer_addr))
    }
}
