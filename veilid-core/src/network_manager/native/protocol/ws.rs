use super::*;

use async_tls::TlsConnector;
use async_tungstenite::tungstenite::error::ProtocolError;
use async_tungstenite::tungstenite::handshake::server::{
    Callback, ErrorResponse, Request, Response,
};
use async_tungstenite::tungstenite::http::StatusCode;
use async_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};
use async_tungstenite::tungstenite::Error;
use async_tungstenite::{accept_hdr_async, client_async, WebSocketStream};
use futures_util::{AsyncRead, AsyncWrite, SinkExt};
use sockets::*;

// Maximum number of websocket request headers to permit
const MAX_WS_HEADERS: usize = 24;
// Maximum size of any one specific websocket header
const MAX_WS_HEADER_LENGTH: usize = 512;
// Maximum total size of headers and request including newlines
const MAX_WS_BEFORE_BODY: usize = 2048;
// Wait time for connection close
// const MAX_CONNECTION_CLOSE_WAIT_US: u64 = 5_000_000;

cfg_if! {
    if #[cfg(feature="rt-async-std")] {
        pub type WebsocketNetworkConnectionWSS =
            WebsocketNetworkConnection<async_tls::client::TlsStream<TcpStream>>;
        pub type WebsocketNetworkConnectionWS = WebsocketNetworkConnection<TcpStream>;
    } else if #[cfg(feature="rt-tokio")] {
        pub type WebsocketNetworkConnectionWSS =
            WebsocketNetworkConnection<async_tls::client::TlsStream<Compat<TcpStream>>>;
        pub type WebsocketNetworkConnectionWS = WebsocketNetworkConnection<Compat<TcpStream>>;
    } else {
        compile_error!("needs executor implementation");
    }
}

fn err_to_network_result<T>(err: Error) -> NetworkResult<T> {
    match err {
        Error::ConnectionClosed
        | Error::AlreadyClosed
        | Error::Io(_)
        | Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)
        | Error::Protocol(ProtocolError::SendAfterClosing) => {
            NetworkResult::NoConnection(to_io_error_other(err))
        }
        _ => NetworkResult::InvalidMessage(err.to_string()),
    }
}

pub type WebSocketNetworkConnectionAccepted = WebsocketNetworkConnection<AsyncPeekStream>;

pub struct WebsocketNetworkConnection<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    flow: Flow,
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
    pub fn new(flow: Flow, stream: WebSocketStream<T>) -> Self {
        Self {
            flow,
            stream: CloneStream::new(stream),
        }
    }

    pub fn flow(&self) -> Flow {
        self.flow
    }

    #[instrument(level = "trace", target = "protocol", err, skip_all)]
    pub async fn close(&self) -> io::Result<NetworkResult<()>> {
        // Make an attempt to close the stream normally
        let mut stream = self.stream.clone();
        let out = match stream
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "".into(),
            })))
            .await
        {
            Ok(v) => NetworkResult::value(v),
            Err(e) => err_to_network_result(e),
        };

        let _ = stream.close().await;

        Ok(out)

        // Drive connection to close
        /*
        let cur_ts = get_timestamp();
        loop {
            match stream.flush().await {
                Ok(()) => {}
                Err(Error::Io(ioerr)) => {
                    break Err(ioerr).into_network_result();
                }
                Err(Error::ConnectionClosed) => {
                    break Ok(NetworkResult::value(()));
                }
                Err(e) => {
                    break Err(to_io_error_other(e));
                }
            }
            if get_timestamp().saturating_sub(cur_ts) >= MAX_CONNECTION_CLOSE_WAIT_US {
                return Ok(NetworkResult::Timeout);
            }
        }
        */
    }

    #[instrument(level = "trace", target="protocol", err, skip(self, message), fields(network_result, message.len = message.len()))]
    pub async fn send(&self, message: Vec<u8>) -> io::Result<NetworkResult<()>> {
        if message.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large WS message");
        }
        let out = match self.stream.clone().send(Message::binary(message)).await {
            Ok(v) => NetworkResult::value(v),
            Err(e) => err_to_network_result(e),
        };

        #[cfg(feature = "verbose-tracing")]
        tracing::Span::current().record("network_result", &tracing::field::display(&out));
        Ok(out)
    }

    #[instrument(level = "trace", target="protocol", err, skip(self), fields(network_result, ret.len))]
    pub async fn recv(&self) -> io::Result<NetworkResult<Vec<u8>>> {
        let out = match self.stream.clone().next().await {
            Some(Ok(Message::Binary(v))) => {
                if v.len() > MAX_MESSAGE_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "too large ws message",
                    ));
                }
                NetworkResult::Value(v)
            }
            Some(Ok(Message::Close(_))) => NetworkResult::NoConnection(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "closeframe",
            )),
            Some(Ok(x)) => NetworkResult::NoConnection(io::Error::new(
                io::ErrorKind::ConnectionReset,
                format!("Unexpected WS message type: {:?}", x),
            )),
            Some(Err(e)) => err_to_network_result(e),
            None => NetworkResult::NoConnection(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "connection ended normally",
            )),
        };

        #[cfg(feature = "verbose-tracing")]
        tracing::Span::current().record("network_result", &tracing::field::display(&out));
        Ok(out)
    }
}

///////////////////////////////////////////////////////////
struct WebsocketProtocolHandlerArc {
    tls: bool,
    request_path: Vec<u8>,
    connection_initial_timeout_ms: u32,
}

#[derive(Clone)]
pub(in crate::network_manager) struct WebsocketProtocolHandler
where
    Self: ProtocolAcceptHandler,
{
    arc: Arc<WebsocketProtocolHandlerArc>,
}
impl WebsocketProtocolHandler {
    pub fn new(config: VeilidConfig, tls: bool) -> Self {
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
                request_path: path.as_bytes().to_vec(),
                connection_initial_timeout_ms,
            }),
        }
    }

    #[instrument(level = "trace", target = "protocol", err, skip(self, ps))]
    pub async fn on_accept_async(
        self,
        ps: AsyncPeekStream,
        socket_addr: SocketAddr,
        local_addr: SocketAddr,
    ) -> io::Result<Option<ProtocolNetworkConnection>> {
        log_net!("WS: on_accept_async: enter");
        let request_path_len = self.arc.request_path.len() + 2;

        let mut peek_buf = [0u8; MAX_WS_BEFORE_BODY];
        let peek_len = match timeout(
            self.arc.connection_initial_timeout_ms,
            ps.peek(&mut peek_buf).in_current_span(),
        )
        .await
        {
            Err(_) => {
                // Timeout
                return Ok(None);
            }
            Ok(Err(_)) => {
                // Peek error
                return Ok(None);
            }
            Ok(Ok(v)) => v,
        };

        // If we can't peek at least our request path, then fail out
        if peek_len < request_path_len {
            return Ok(None);
        }

        // Check for websocket path
        let matches_path = &peek_buf[0..request_path_len - 2] == self.arc.request_path.as_slice()
            && (peek_buf[request_path_len - 2] == b' '
                || (peek_buf[request_path_len - 2] == b'/'
                    && peek_buf[request_path_len - 1] == b' '));

        if !matches_path {
            return Ok(None);
        }

        // Check for double-CRLF indicating end of headers
        // if we don't find the end of the headers within MAX_WS_BEFORE_BODY
        // then we should bail, as this could be an attack or at best, something malformed
        // Yes, this restricts our handling to CRLF-conforming HTTP implementations
        // This check could be loosened if necessary, but until we have a reason to do so
        // a stricter interpretation of HTTP is possible and desirable to reduce attack surface

        if !peek_buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(None);
        }

        let ws_stream = match accept_hdr_async(ps, self.clone()).await {
            Ok(v) => v,
            Err(e) => {
                log_net!(debug "failed websockets handshake: {}", e);
                return Ok(None);
            }
        };

        // Wrap the websocket in a NetworkConnection and register it
        let protocol_type = if self.arc.tls {
            ProtocolType::WSS
        } else {
            ProtocolType::WS
        };

        let peer_addr =
            PeerAddress::new(SocketAddress::from_socket_addr(socket_addr), protocol_type);

        let conn = ProtocolNetworkConnection::WsAccepted(WebsocketNetworkConnection::new(
            Flow::new(peer_addr, SocketAddress::from_socket_addr(local_addr)),
            ws_stream,
        ));

        log_net!(
            "Connection accepted from: {} ({})",
            socket_addr,
            if self.arc.tls { "WSS" } else { "WS" }
        );

        Ok(Some(conn))
    }

    #[instrument(level = "trace", target = "protocol", ret, err)]
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: &DialInfo,
        timeout_ms: u32,
    ) -> io::Result<NetworkResult<ProtocolNetworkConnection>> {
        // Split dial info up
        let (tls, scheme) = match dial_info {
            DialInfo::WS(_) => (false, "ws"),
            DialInfo::WSS(_) => (true, "wss"),
            _ => panic!("invalid dialinfo for WS/WSS protocol"),
        };
        let request = dial_info.request().unwrap();
        let split_url = SplitUrl::from_str(&request).map_err(to_io_error_other)?;
        if split_url.scheme != scheme {
            bail_io_error_other!("invalid websocket url scheme");
        }
        let domain = split_url.host.clone();

        // Resolve remote address
        let remote_socket_addr = dial_info.to_socket_addr();

        // Make a shared socket
        let socket = match local_address {
            Some(a) => {
                new_bound_shared_tcp_socket(a)?.ok_or(io::Error::from(io::ErrorKind::AddrInUse))?
            }
            None => new_default_tcp_socket(socket2::Domain::for_address(remote_socket_addr))?,
        };

        // Non-blocking connect to remote address
        let tcp_stream =
            network_result_try!(nonblocking_connect(socket, remote_socket_addr, timeout_ms)
                .await
                .folded()?);

        // See what local address we ended up with
        let actual_local_addr = tcp_stream.local_addr()?;

        #[cfg(feature = "rt-tokio")]
        let tcp_stream = tcp_stream.compat();

        // Make our flow
        let flow = Flow::new(
            dial_info.peer_address(),
            SocketAddress::from_socket_addr(actual_local_addr),
        );
        log_net!("{}::connect: {:?}", scheme, flow);

        // Negotiate TLS if this is WSS
        if tls {
            let connector = TlsConnector::default();
            let tls_stream = network_result_try!(connector
                .connect(domain.to_string(), tcp_stream)
                .await
                .into_network_result()?);
            let (ws_stream, _response) = client_async(request, tls_stream)
                .await
                .map_err(to_io_error_other)?;

            Ok(NetworkResult::Value(ProtocolNetworkConnection::Wss(
                WebsocketNetworkConnection::new(flow, ws_stream),
            )))
        } else {
            let (ws_stream, _response) = client_async(request, tcp_stream)
                .await
                .map_err(to_io_error_other)?;
            Ok(NetworkResult::Value(ProtocolNetworkConnection::Ws(
                WebsocketNetworkConnection::new(flow, ws_stream),
            )))
        }
    }
}

impl Callback for WebsocketProtocolHandler {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        // Cap the number of headers total and limit the size of all headers
        if request.headers().len() > MAX_WS_HEADERS
            || request
                .headers()
                .iter()
                .any(|h| (h.0.as_str().len() + h.1.as_bytes().len()) > MAX_WS_HEADER_LENGTH)
        {
            let mut error_response = ErrorResponse::new(None);
            *error_response.status_mut() = StatusCode::NOT_FOUND;
            return Err(error_response);
        }
        Ok(response)
    }
}

impl ProtocolAcceptHandler for WebsocketProtocolHandler {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
    ) -> SendPinBoxFuture<io::Result<Option<ProtocolNetworkConnection>>> {
        Box::pin(self.clone().on_accept_async(stream, peer_addr, local_addr))
    }
}
