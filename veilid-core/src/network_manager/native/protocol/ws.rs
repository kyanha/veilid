use super::*;

use async_tls::TlsConnector;
use async_tungstenite::tungstenite::protocol::Message;
use async_tungstenite::{accept_async, client_async, WebSocketStream};
use futures_util::{AsyncRead, AsyncWrite, SinkExt};
use sockets::*;
cfg_if! {
    if #[cfg(feature="rt-async-std")] {
        pub type WebsocketNetworkConnectionWSS =
            WebsocketNetworkConnection<async_tls::client::TlsStream<TcpStream>>;
        pub type WebsocketNetworkConnectionWS = WebsocketNetworkConnection<TcpStream>;
    } else if #[cfg(feature="rt-tokio")] {
        pub type WebsocketNetworkConnectionWSS =
            WebsocketNetworkConnection<async_tls::client::TlsStream<Compat<TcpStream>>>;
        pub type WebsocketNetworkConnectionWS = WebsocketNetworkConnection<Compat<TcpStream>>;
    }
}

pub type WebSocketNetworkConnectionAccepted = WebsocketNetworkConnection<AsyncPeekStream>;

pub struct WebsocketNetworkConnection<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    descriptor: ConnectionDescriptor,
    stream: CloneStream<WebSocketStream<T>>,
}

impl<T> fmt::Debug for WebsocketNetworkConnection<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::any::type_name::<Self>())
    }
}

impl<T> WebsocketNetworkConnection<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    pub fn new(descriptor: ConnectionDescriptor, stream: WebSocketStream<T>) -> Self {
        Self {
            descriptor,
            stream: CloneStream::new(stream),
        }
    }

    pub fn descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }

    // #[instrument(level = "trace", err, skip(self))]
    // pub async fn close(&self) -> Result<(), String> {
    //     // Make an attempt to flush the stream
    //     self.stream.clone().close().await.map_err(map_to_string)?;
    //     // Then forcibly close the socket
    //     self.tcp_stream
    //         .shutdown(Shutdown::Both)
    //         .map_err(map_to_string)
    // }

    #[instrument(level = "trace", err, skip(self, message), fields(message.len = message.len()))]
    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        if message.len() > MAX_MESSAGE_SIZE {
            return Err("received too large WS message".to_owned());
        }
        self.stream
            .clone()
            .send(Message::binary(message))
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed to send websocket message"))
    }

    #[instrument(level = "trace", err, skip(self), fields(ret.len))]
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        let out = match self.stream.clone().next().await {
            Some(Ok(Message::Binary(v))) => v,
            Some(Ok(x)) => {
                return Err(format!("Unexpected WS message type: {:?}", x));
            }
            Some(Err(e)) => {
                return Err(e.to_string()).map_err(logthru_net!(error));
            }
            None => {
                return Err("WS stream closed".to_owned());
            }
        };
        if out.len() > MAX_MESSAGE_SIZE {
            Err("sending too large WS message".to_owned()).map_err(logthru_net!(error))
        } else {
            tracing::Span::current().record("ret.len", &out.len());
            Ok(out)
        }
    }
}

///////////////////////////////////////////////////////////
///
struct WebsocketProtocolHandlerArc {
    tls: bool,
    local_address: SocketAddr,
    request_path: Vec<u8>,
    connection_initial_timeout_ms: u32,
}

#[derive(Clone)]
pub struct WebsocketProtocolHandler
where
    Self: ProtocolAcceptHandler,
{
    arc: Arc<WebsocketProtocolHandlerArc>,
}
impl WebsocketProtocolHandler {
    pub fn new(config: VeilidConfig, tls: bool, local_address: SocketAddr) -> Self {
        let c = config.get();
        let path = if tls {
            format!("GET /{}", c.network.protocol.ws.path.trim_end_matches('/'))
        } else {
            format!("GET /{}", c.network.protocol.wss.path.trim_end_matches('/'))
        };
        let connection_initial_timeout_ms = if tls {
            c.network.tls.connection_initial_timeout_ms
        } else {
            c.network.connection_initial_timeout_ms
        };

        Self {
            arc: Arc::new(WebsocketProtocolHandlerArc {
                tls,
                local_address,
                request_path: path.as_bytes().to_vec(),
                connection_initial_timeout_ms,
            }),
        }
    }

    #[instrument(level = "trace", err, skip(self, ps))]
    pub async fn on_accept_async(
        self,
        ps: AsyncPeekStream,
        socket_addr: SocketAddr,
    ) -> Result<Option<ProtocolNetworkConnection>, String> {
        log_net!("WS: on_accept_async: enter");
        let request_path_len = self.arc.request_path.len() + 2;

        let mut peekbuf: Vec<u8> = vec![0u8; request_path_len];
        match timeout(
            self.arc.connection_initial_timeout_ms,
            ps.peek_exact(&mut peekbuf),
        )
        .await
        {
            Ok(_) => (),
            Err(e) => {
                return Err(e.to_string());
            }
        }

        // Check for websocket path
        let matches_path = &peekbuf[0..request_path_len - 2] == self.arc.request_path.as_slice()
            && (peekbuf[request_path_len - 2] == b' '
                || (peekbuf[request_path_len - 2] == b'/'
                    && peekbuf[request_path_len - 1] == b' '));

        if !matches_path {
            log_net!("WS: not websocket");
            return Ok(None);
        }
        log_net!("WS: found websocket");

        let ws_stream = accept_async(ps)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!("failed websockets handshake"))?;

        // Wrap the websocket in a NetworkConnection and register it
        let protocol_type = if self.arc.tls {
            ProtocolType::WSS
        } else {
            ProtocolType::WS
        };

        let peer_addr =
            PeerAddress::new(SocketAddress::from_socket_addr(socket_addr), protocol_type);

        let conn = ProtocolNetworkConnection::WsAccepted(WebsocketNetworkConnection::new(
            ConnectionDescriptor::new(
                peer_addr,
                SocketAddress::from_socket_addr(self.arc.local_address),
            ),
            ws_stream,
        ));

        log_net!(debug "{}: on_accept_async from: {}", if self.arc.tls { "WSS" } else { "WS" }, socket_addr);

        Ok(Some(conn))
    }

    async fn connect_internal(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ProtocolNetworkConnection, String> {
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
        let socket = match local_address {
            Some(a) => new_bound_shared_tcp_socket(a)?,
            None => {
                new_unbound_shared_tcp_socket(socket2::Domain::for_address(remote_socket_addr))?
            }
        };

        // Non-blocking connect to remote address
        let tcp_stream = nonblocking_connect(socket, remote_socket_addr).await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "local_address={:?} remote_addr={}", local_address, remote_socket_addr))?;

        // See what local address we ended up with
        let actual_local_addr = tcp_stream.local_addr().map_err(map_to_string)?;

        #[cfg(feature = "rt-tokio")]
        let tcp_stream = tcp_stream.compat();

        // Make our connection descriptor
        let descriptor = ConnectionDescriptor::new(
            dial_info.to_peer_address(),
            SocketAddress::from_socket_addr(actual_local_addr),
        );
        // Negotiate TLS if this is WSS
        if tls {
            let connector = TlsConnector::default();
            let tls_stream = connector
                .connect(domain.to_string(), tcp_stream)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;
            let (ws_stream, _response) = client_async(request, tls_stream)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;

            Ok(ProtocolNetworkConnection::Wss(
                WebsocketNetworkConnection::new(descriptor, ws_stream),
            ))
        } else {
            let (ws_stream, _response) = client_async(request, tcp_stream)
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;
            Ok(ProtocolNetworkConnection::Ws(
                WebsocketNetworkConnection::new(descriptor, ws_stream),
            ))
        }
    }

    #[instrument(level = "trace", err)]
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ProtocolNetworkConnection, String> {
        Self::connect_internal(local_address, dial_info).await
    }

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len()))]
    pub async fn send_unbound_message(dial_info: DialInfo, data: Vec<u8>) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound WS message".to_owned());
        }

        let protconn = Self::connect_internal(None, dial_info.clone())
            .await
            .map_err(|e| format!("failed to connect websocket for unbound message: {}", e))?;

        protconn.send(data).await
    }

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len(), ret.len))]
    pub async fn send_recv_unbound_message(
        dial_info: DialInfo,
        data: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<Vec<u8>, String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound WS message".to_owned());
        }

        let protconn = Self::connect_internal(None, dial_info.clone())
            .await
            .map_err(|e| format!("failed to connect websocket for unbound message: {}", e))?;

        protconn.send(data).await?;
        let out = timeout(timeout_ms, protconn.recv())
            .await
            .map_err(map_to_string)??;

        tracing::Span::current().record("ret.len", &out.len());
        Ok(out)
    }
}

impl ProtocolAcceptHandler for WebsocketProtocolHandler {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
    ) -> SystemPinBoxFuture<Result<Option<ProtocolNetworkConnection>, String>> {
        Box::pin(self.clone().on_accept_async(stream, peer_addr))
    }
}
