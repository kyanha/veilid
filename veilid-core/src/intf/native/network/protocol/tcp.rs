use super::*;
use crate::intf::native::utils::async_peek_stream::*;
use crate::intf::*;
use crate::network_manager::{NetworkManager, MAX_MESSAGE_SIZE};
use crate::*;
use async_std::net::*;
use async_std::prelude::*;
use async_std::sync::Mutex as AsyncMutex;
use std::fmt;

struct RawTcpNetworkConnectionInner {
    stream: AsyncPeekStream,
}

#[derive(Clone)]
pub struct RawTcpNetworkConnection {
    inner: Arc<AsyncMutex<RawTcpNetworkConnectionInner>>,
}

impl fmt::Debug for RawTcpNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::any::type_name::<Self>())
    }
}

impl PartialEq for RawTcpNetworkConnection {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.inner) == Arc::as_ptr(&other.inner)
    }
}

impl Eq for RawTcpNetworkConnection {}

impl RawTcpNetworkConnection {
    fn new_inner(stream: AsyncPeekStream) -> RawTcpNetworkConnectionInner {
        RawTcpNetworkConnectionInner { stream }
    }

    pub fn new(stream: AsyncPeekStream) -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(Self::new_inner(stream))),
        }
    }
}

impl RawTcpNetworkConnection {
    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), String>> {
        let inner = self.inner.clone();

        Box::pin(async move {
            if message.len() > MAX_MESSAGE_SIZE {
                return Err("sending too large TCP message".to_owned());
            }
            let len = message.len() as u16;
            let header = [b'V', b'L', len as u8, (len >> 8) as u8];

            let mut inner = inner.lock().await;
            inner
                .stream
                .write_all(&header)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!())?;
            inner
                .stream
                .write_all(&message)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!())
        })
    }

    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, String>> {
        let inner = self.inner.clone();

        Box::pin(async move {
            let mut header = [0u8; 4];
            let mut inner = inner.lock().await;

            inner
                .stream
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
            inner
                .stream
                .read_exact(&mut out)
                .await
                .map_err(map_to_string)?;
            Ok(out)
        })
    }
}

///////////////////////////////////////////////////////////
///

struct RawTcpProtocolHandlerInner {
    network_manager: NetworkManager,
    local_address: SocketAddr,
}

#[derive(Clone)]
pub struct RawTcpProtocolHandler
where
    Self: TcpProtocolHandler,
{
    inner: Arc<Mutex<RawTcpProtocolHandlerInner>>,
}

impl RawTcpProtocolHandler {
    fn new_inner(
        network_manager: NetworkManager,
        local_address: SocketAddr,
    ) -> RawTcpProtocolHandlerInner {
        RawTcpProtocolHandlerInner {
            network_manager,
            local_address,
        }
    }

    pub fn new(network_manager: NetworkManager, local_address: SocketAddr) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager, local_address))),
        }
    }

    pub async fn on_accept_async(
        self,
        stream: AsyncPeekStream,
        socket_addr: SocketAddr,
    ) -> Result<bool, String> {
        let mut peekbuf: [u8; PEEK_DETECT_LEN] = [0u8; PEEK_DETECT_LEN];
        let peeklen = stream
            .peek(&mut peekbuf)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!("could not peek tcp stream"))?;
        assert_eq!(peeklen, PEEK_DETECT_LEN);

        let conn = NetworkConnection::RawTcp(RawTcpNetworkConnection::new(stream));
        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(socket_addr),
            ProtocolType::TCP,
        );
        let (network_manager, local_address) = {
            let inner = self.inner.lock();
            (inner.network_manager.clone(), inner.local_address)
        };
        network_manager
            .on_new_connection(
                ConnectionDescriptor::new(
                    peer_addr,
                    SocketAddress::from_socket_addr(local_address),
                ),
                conn,
            )
            .await?;
        Ok(true)
    }

    pub async fn connect(
        network_manager: NetworkManager,
        local_address: SocketAddr,
        remote_socket_addr: SocketAddr,
    ) -> Result<NetworkConnection, String> {
        // Make a shared socket
        let socket = new_shared_tcp_socket(local_address)?;

        // Connect to the remote address
        let remote_socket2_addr = socket2::SockAddr::from(remote_socket_addr);
        socket
            .connect(&remote_socket2_addr)
            .map_err(map_to_string)
            .map_err(logthru_net!(error "addr={}", remote_socket_addr))?;
        let std_stream: std::net::TcpStream = socket.into();
        let ts = TcpStream::from(std_stream);

        // See what local address we ended up with and turn this into a stream
        let local_address = ts
            .local_addr()
            .map_err(map_to_string)
            .map_err(logthru_net!())?;
        let ps = AsyncPeekStream::new(ts);
        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(remote_socket_addr),
            ProtocolType::TCP,
        );

        // Wrap the stream in a network connection and register it
        let conn = NetworkConnection::RawTcp(RawTcpNetworkConnection::new(ps));
        network_manager
            .on_new_connection(
                ConnectionDescriptor::new(
                    peer_addr,
                    SocketAddress::from_socket_addr(local_address),
                ),
                conn.clone(),
            )
            .await?;
        Ok(conn)
    }

    pub async fn send_unbound_message(
        data: Vec<u8>,
        socket_addr: SocketAddr,
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
            .map_err(|e| format!("{}", e))?;
        stream.write_all(&data).await.map_err(|e| format!("{}", e))
    }
}

impl TcpProtocolHandler for RawTcpProtocolHandler {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
    ) -> SendPinBoxFuture<Result<bool, String>> {
        Box::pin(self.clone().on_accept_async(stream, peer_addr))
    }
}
