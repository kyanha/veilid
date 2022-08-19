use super::*;
use futures_util::{SinkExt, StreamExt};
use send_wrapper::*;
use std::io;
use ws_stream_wasm::*;

struct WebsocketNetworkConnectionInner {
    _ws_meta: WsMeta,
    ws_stream: CloneStream<WsStream>,
}

fn to_io(err: WsErr) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}

#[derive(Clone)]
pub struct WebsocketNetworkConnection {
    descriptor: ConnectionDescriptor,
    inner: Arc<WebsocketNetworkConnectionInner>,
}

impl fmt::Debug for WebsocketNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", core::any::type_name::<Self>())
    }
}

impl WebsocketNetworkConnection {
    pub fn new(descriptor: ConnectionDescriptor, ws_meta: WsMeta, ws_stream: WsStream) -> Self {
        Self {
            descriptor,
            inner: Arc::new(WebsocketNetworkConnectionInner {
                _ws_meta: ws_meta,
                ws_stream: CloneStream::new(ws_stream),
            }),
        }
    }

    pub fn descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }

    // #[instrument(level = "trace", err, skip(self))]
    // pub async fn close(&self) -> io::Result<()> {
    //     self.inner.ws_meta.close().await.map_err(to_io).map(drop)
    // }

    #[instrument(level = "trace", err, skip(self, message), fields(network_result, message.len = message.len()))]
    pub async fn send(&self, message: Vec<u8>) -> io::Result<NetworkResult<()>> {
        if message.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large WS message");
        }
        let out = SendWrapper::new(
            self.inner
                .ws_stream
                .clone()
                .send(WsMessage::Binary(message)),
        )
        .await
        .map_err(to_io)
        .into_network_result()?;

        tracing::Span::current().record("network_result", &tracing::field::display(&out));
        Ok(out)
    }

    #[instrument(level = "trace", err, skip(self), fields(network_result, ret.len))]
    pub async fn recv(&self) -> io::Result<NetworkResult<Vec<u8>>> {
        let out = match SendWrapper::new(self.inner.ws_stream.clone().next()).await {
            Some(WsMessage::Binary(v)) => {
                if v.len() > MAX_MESSAGE_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "too large ws message",
                    ));
                }
                NetworkResult::Value(v)
            }
            Some(_) => NetworkResult::NoConnection(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "Unexpected WS message type",
            )),
            None => {
                bail_io_error_other!("WS stream closed");
            }
        };
        tracing::Span::current().record("network_result", &tracing::field::display(&out));
        Ok(out)
    }
}

///////////////////////////////////////////////////////////
///

pub struct WebsocketProtocolHandler {}

impl WebsocketProtocolHandler {
    #[instrument(level = "trace", err)]
    pub async fn connect(
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

        let fut = SendWrapper::new(timeout(timeout_ms, async move {
            WsMeta::connect(request, None).await.map_err(to_io)
        }));
        let (wsmeta, wsio) = network_result_try!(network_result_try!(fut
            .await
            .into_network_result())
        .into_network_result()?);

        // Make our connection descriptor

        let wnc = WebsocketNetworkConnection::new(
            ConnectionDescriptor::new_no_local(dial_info.to_peer_address())
                .map_err(|e| io::Error::new(io::ErrorKind::AddrNotAvailable, e))?,
            wsmeta,
            wsio,
        );

        Ok(NetworkResult::Value(ProtocolNetworkConnection::Ws(wnc)))
    }
}
