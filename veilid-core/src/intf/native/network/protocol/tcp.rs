use super::*;
use crate::intf::*;
use crate::network_manager::MAX_MESSAGE_SIZE;
use crate::*;
use async_std::net::TcpStream;
use core::fmt;
use futures_util::{AsyncReadExt, AsyncWriteExt};

pub struct RawTcpNetworkConnection {
    stream: AsyncPeekStream,
}

impl fmt::Debug for RawTcpNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawTCPNetworkConnection").finish()
    }
}

impl RawTcpNetworkConnection {
    pub fn new(stream: AsyncPeekStream) -> Self {
        Self { stream }
    }

    pub async fn close(&self) -> Result<(), String> {
        self.stream
            .clone()
            .close()
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!())
    }

    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        log_net!("sending TCP message of size {}", message.len());
        if message.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large TCP message".to_owned());
        }
        let len = message.len() as u16;
        let header = [b'V', b'L', len as u8, (len >> 8) as u8];

        let mut stream = self.stream.clone();

        stream
            .write_all(&header)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!())?;
        stream
            .write_all(&message)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!())
    }

    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        let mut header = [0u8; 4];

        let mut stream = self.stream.clone();

        stream
            .read_exact(&mut header)
            .await
            .map_err(|e| format!("TCP recv error: {}", e))?;
        if header[0] != b'V' || header[1] != b'L' {
            return Err("received invalid TCP frame header".to_owned());
        }
        let len = ((header[3] as usize) << 8) | (header[2] as usize);
        if len > MAX_MESSAGE_SIZE {
            return Err("received too large TCP frame".to_owned());
        }

        let mut out: Vec<u8> = vec![0u8; len];
        stream.read_exact(&mut out).await.map_err(map_to_string)?;
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

    async fn on_accept_async(
        self,
        stream: AsyncPeekStream,
        socket_addr: SocketAddr,
    ) -> Result<Option<NetworkConnection>, String> {
        let mut peekbuf: [u8; PEEK_DETECT_LEN] = [0u8; PEEK_DETECT_LEN];
        let peeklen = stream
            .peek(&mut peekbuf)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!("could not peek tcp stream"))?;
        assert_eq!(peeklen, PEEK_DETECT_LEN);

        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(socket_addr),
            ProtocolType::TCP,
        );
        let local_address = self.inner.lock().local_address;
        let conn = NetworkConnection::from_protocol(
            ConnectionDescriptor::new(peer_addr, SocketAddress::from_socket_addr(local_address)),
            ProtocolNetworkConnection::RawTcp(RawTcpNetworkConnection::new(stream)),
        );

        warn!("on_accept_async from: {}", socket_addr);

        Ok(Some(conn))
    }

    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<NetworkConnection, String> {
        // Get remote socket address to connect to
        let remote_socket_addr = dial_info.to_socket_addr();

        // Make a shared socket
        let socket = match local_address {
            Some(a) => new_bound_shared_tcp_socket(a)?,
            None => new_unbound_shared_tcp_socket(Domain::for_address(remote_socket_addr))?,
        };

        // Connect to the remote address
        let remote_socket2_addr = socket2::SockAddr::from(remote_socket_addr);
        socket
            .connect(&remote_socket2_addr)
            .map_err(map_to_string)
            .map_err(logthru_net!(error "local_address={:?} remote_addr={}", local_address, remote_socket_addr))?;

        let std_stream: std::net::TcpStream = socket.into();
        let ts = TcpStream::from(std_stream);

        // See what local address we ended up with and turn this into a stream
        let actual_local_address = ts
            .local_addr()
            .map_err(map_to_string)
            .map_err(logthru_net!("could not get local address from TCP stream"))?;
        let ps = AsyncPeekStream::new(ts);

        // Wrap the stream in a network connection and return it
        let conn = NetworkConnection::from_protocol(
            ConnectionDescriptor {
                local: Some(SocketAddress::from_socket_addr(actual_local_address)),
                remote: dial_info.to_peer_address(),
            },
            ProtocolNetworkConnection::RawTcp(RawTcpNetworkConnection::new(ps)),
        );
        Ok(conn)
    }

    pub async fn send_unbound_message(
        socket_addr: SocketAddr,
        data: Vec<u8>,
    ) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound TCP message".to_owned());
        }
        trace!(
            "sending unbound message of length {} to {}",
            data.len(),
            socket_addr
        );

        let mut stream = TcpStream::connect(socket_addr)
            .await
            .map_err(|e| format!("failed to connect TCP for unbound message: {}", e))?;
        stream.write_all(&data).await.map_err(|e| format!("{}", e))
    }
}

impl ProtocolAcceptHandler for RawTcpProtocolHandler {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
    ) -> SystemPinBoxFuture<core::result::Result<Option<NetworkConnection>, String>> {
        Box::pin(self.clone().on_accept_async(stream, peer_addr))
    }
}
