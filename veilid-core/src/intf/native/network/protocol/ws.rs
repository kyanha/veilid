use super::*;
use crate::intf::native::utils::async_peek_stream::*;
use crate::intf::*;
use crate::network_manager::{NetworkManager, MAX_MESSAGE_SIZE};
use crate::*;
use async_std::io;
use async_std::net::*;
use async_std::sync::Mutex as AsyncMutex;
use async_tls::TlsConnector;
use async_tungstenite::tungstenite::protocol::Message;
use async_tungstenite::{accept_async, client_async, WebSocketStream};
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

pub type WebSocketNetworkConnectionAccepted = WebsocketNetworkConnection<AsyncPeekStream>;
pub type WebsocketNetworkConnectionWSS =
    WebsocketNetworkConnection<async_tls::client::TlsStream<async_std::net::TcpStream>>;
pub type WebsocketNetworkConnectionWS = WebsocketNetworkConnection<async_std::net::TcpStream>;

struct WebSocketNetworkConnectionInner<T>
where
    T: io::Read + io::Write + Send + Unpin + 'static,
{
    ws_stream: WebSocketStream<T>,
}

pub struct WebsocketNetworkConnection<T>
where
    T: io::Read + io::Write + Send + Unpin + 'static,
{
    tls: bool,
    inner: Arc<AsyncMutex<WebSocketNetworkConnectionInner<T>>>,
}

impl<T> Clone for WebsocketNetworkConnection<T>
where
    T: io::Read + io::Write + Send + Unpin + 'static,
{
    fn clone(&self) -> Self {
        Self {
            tls: self.tls,
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for WebsocketNetworkConnection<T>
where
    T: io::Read + io::Write + Send + Unpin + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::any::type_name::<Self>())
    }
}

impl<T> PartialEq for WebsocketNetworkConnection<T>
where
    T: io::Read + io::Write + Send + Unpin + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.tls == other.tls && Arc::as_ptr(&self.inner) == Arc::as_ptr(&other.inner)
    }
}

impl<T> Eq for WebsocketNetworkConnection<T> where T: io::Read + io::Write + Send + Unpin + 'static {}

impl<T> WebsocketNetworkConnection<T>
where
    T: io::Read + io::Write + Send + Unpin + 'static,
{
    pub fn new(tls: bool, ws_stream: WebSocketStream<T>) -> Self {
        Self {
            tls,
            inner: Arc::new(AsyncMutex::new(WebSocketNetworkConnectionInner {
                ws_stream,
            })),
        }
    }

    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), String>> {
        let inner = self.inner.clone();

        Box::pin(async move {
            if message.len() > MAX_MESSAGE_SIZE {
                return Err("received too large WS message".to_owned());
            }
            let mut inner = inner.lock().await;
            inner
                .ws_stream
                .send(Message::binary(message))
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error "failed to send websocket message"))
        })
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, String>> {
        let inner = self.inner.clone();

        Box::pin(async move {
            let mut inner = inner.lock().await;

            let out = match inner.ws_stream.next().await {
                Some(Ok(Message::Binary(v))) => v,
                Some(Ok(_)) => {
                    return Err("Unexpected WS message type".to_owned())
                        .map_err(logthru_net!(error));
                }
                Some(Err(e)) => {
                    return Err(e.to_string()).map_err(logthru_net!(error));
                }
                None => {
                    return Err("WS stream closed".to_owned()).map_err(logthru_net!());
                }
            };
            if out.len() > MAX_MESSAGE_SIZE {
                Err("sending too large WS message".to_owned()).map_err(logthru_net!(error))
            } else {
                Ok(out)
            }
        })
    }
}

///////////////////////////////////////////////////////////
///
struct WebsocketProtocolHandlerInner {
    tls: bool,
    network_manager: NetworkManager,
    local_address: SocketAddr,
    request_path: Vec<u8>,
    connection_initial_timeout: u64,
}

#[derive(Clone)]
pub struct WebsocketProtocolHandler
where
    Self: TcpProtocolHandler,
{
    inner: Arc<WebsocketProtocolHandlerInner>,
}
impl WebsocketProtocolHandler {
    pub fn new(network_manager: NetworkManager, tls: bool, local_address: SocketAddr) -> Self {
        let config = network_manager.config();
        let c = config.get();
        let path = if tls {
            format!("GET {}", c.network.protocol.ws.path.trim_end_matches('/'))
        } else {
            format!("GET {}", c.network.protocol.wss.path.trim_end_matches('/'))
        };
        let connection_initial_timeout = if tls {
            c.network.tls.connection_initial_timeout
        } else {
            c.network.connection_initial_timeout
        };

        let inner = WebsocketProtocolHandlerInner {
            tls,
            network_manager,
            local_address,
            request_path: path.as_bytes().to_vec(),
            connection_initial_timeout,
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub async fn on_accept_async(
        self,
        ps: AsyncPeekStream,
        socket_addr: SocketAddr,
    ) -> Result<bool, String> {
        let request_path_len = self.inner.request_path.len() + 2;
        let mut peekbuf: Vec<u8> = vec![0u8; request_path_len];
        match io::timeout(
            Duration::from_micros(self.inner.connection_initial_timeout),
            ps.peek_exact(&mut peekbuf),
        )
        .await
        {
            Ok(_) => (),
            Err(e) => {
                if e.kind() == io::ErrorKind::TimedOut {
                    return Err(e).map_err(map_to_string).map_err(logthru_net!());
                }
                return Err(e).map_err(map_to_string).map_err(logthru_net!(error));
            }
        }
        // Check for websocket path
        let matches_path = &peekbuf[0..request_path_len - 2] == self.inner.request_path.as_slice()
            && (peekbuf[request_path_len - 2] == b' '
                || (peekbuf[request_path_len - 2] == b'/'
                    && peekbuf[request_path_len - 1] == b' '));

        if !matches_path {
            log_net!("not websocket");
            return Ok(false);
        }
        log_net!("found websocket");

        let ws_stream = accept_async(ps)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!("failed websockets handshake"))?;

        // Wrap the websocket in a NetworkConnection and register it
        let protocol_type = if self.inner.tls {
            ProtocolType::WSS
        } else {
            ProtocolType::WS
        };

        let peer_addr =
            PeerAddress::new(SocketAddress::from_socket_addr(socket_addr), protocol_type);

        let conn = NetworkConnection::WsAccepted(WebsocketNetworkConnection::new(
            self.inner.tls,
            ws_stream,
        ));
        self.inner
            .network_manager
            .clone()
            .on_new_connection(
                ConnectionDescriptor::new(
                    peer_addr,
                    SocketAddress::from_socket_addr(self.inner.local_address),
                ),
                conn,
            )
            .await?;
        Ok(true)
    }

    pub async fn connect(
        network_manager: NetworkManager,
        local_address: SocketAddr,
        dial_info: &DialInfo,
    ) -> Result<NetworkConnection, String> {
        // Split dial info up
        let (tls, scheme) = match &dial_info {
            DialInfo::WS(_) => (false, "ws"),
            DialInfo::WSS(_) => (true, "wss"),
            _ => panic!("invalid dialinfo for WS/WSS protocol"),
        };
        let request = dial_info.request().unwrap();
        let split_url = SplitUrl::from_str(&request)?;
        if split_url.scheme != scheme {
            return Err("invalid websocket url scheme".to_string());
        }
        let domain = split_url.host.clone();

        // Resolve remote address
        let remote_socket_addr = dial_info.to_socket_addr();

        // Make a shared socket
        let socket = new_shared_tcp_socket(local_address)?;

        // Connect to the remote address
        let remote_socket2_addr = socket2::SockAddr::from(remote_socket_addr);
        socket
            .connect(&remote_socket2_addr)
            .map_err(map_to_string)
            .map_err(logthru_net!(error "local_address={} remote_socket_addr={}", local_address, remote_socket_addr))?;
        let std_stream: std::net::TcpStream = socket.into();
        let tcp_stream = TcpStream::from(std_stream);

        // See what local address we ended up with
        let actual_local_addr = tcp_stream
            .local_addr()
            .map_err(map_to_string)
            .map_err(logthru_net!())?;

        // Negotiate TLS if this is WSS
        if tls {
            let connector = TlsConnector::default();
            let tls_stream = connector
                .connect(domain, tcp_stream)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;
            let (ws_stream, _response) = client_async(request, tls_stream)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;
            let conn = NetworkConnection::Wss(WebsocketNetworkConnection::new(tls, ws_stream));

            // Make the connection descriptor peer address
            let peer_addr = PeerAddress::new(
                SocketAddress::from_socket_addr(remote_socket_addr),
                ProtocolType::WSS,
            );

            // Register the WSS connection
            network_manager
                .on_new_connection(
                    ConnectionDescriptor::new(
                        peer_addr,
                        SocketAddress::from_socket_addr(actual_local_addr),
                    ),
                    conn.clone(),
                )
                .await?;
            Ok(conn)
        } else {
            let (ws_stream, _response) = client_async(request, tcp_stream)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;
            let conn = NetworkConnection::Ws(WebsocketNetworkConnection::new(tls, ws_stream));

            // Make the connection descriptor peer address
            let peer_addr = PeerAddress::new(
                SocketAddress::from_socket_addr(remote_socket_addr),
                ProtocolType::WS,
            );

            // Register the WS connection
            network_manager
                .on_new_connection(
                    ConnectionDescriptor::new(
                        peer_addr,
                        SocketAddress::from_socket_addr(actual_local_addr),
                    ),
                    conn.clone(),
                )
                .await?;
            Ok(conn)
        }
    }
}

impl TcpProtocolHandler for WebsocketProtocolHandler {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
    ) -> SystemPinBoxFuture<Result<bool, String>> {
        Box::pin(self.clone().on_accept_async(stream, peer_addr))
    }
}
