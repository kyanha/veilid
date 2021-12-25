use crate::intf::*;
use crate::network_manager::{NetworkManager, MAX_MESSAGE_SIZE};
use crate::*;
use alloc::fmt;
use futures_util::stream::StreamExt;
use web_sys::WebSocket;
use ws_stream_wasm::*;

struct WebsocketNetworkConnectionInner {
    ws_stream: WsStream,
    ws: WebSocket,
}

#[derive(Clone)]
pub struct WebsocketNetworkConnection {
    tls: bool,
    inner: Arc<Mutex<WebsocketNetworkConnectionInner>>,
}

impl fmt::Debug for WebsocketNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", core::any::type_name::<Self>())
    }
}

impl PartialEq for WebsocketNetworkConnection {
    fn eq(&self, other: &Self) -> bool {
        self.tls == other.tls && Arc::as_ptr(&self.inner) == Arc::as_ptr(&other.inner)
    }
}

impl Eq for WebsocketNetworkConnection {}

impl WebsocketNetworkConnection {
    pub fn new(tls: bool, ws_meta: WsMeta, ws_stream: WsStream) -> Self {
        let ws = ws_stream.wrapped().clone();
        Self {
            tls,
            inner: Arc::new(Mutex::new(WebsocketNetworkConnectionInner {
                ws_stream,
                ws,
            })),
        }
    }
}

impl WebsocketNetworkConnection {
    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), String>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            if message.len() > MAX_MESSAGE_SIZE {
                return Err("sending too large WS message".to_owned()).map_err(logthru_net!(error));
            }
            inner
                .lock()
                .ws
                .send_with_u8_array(&message)
                .map_err(|_| "failed to send to websocket".to_owned())
                .map_err(logthru_net!(error))
        })
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, String>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let out = match inner.lock().ws_stream.next().await {
                Some(WsMessage::Binary(v)) => v,
                Some(_) => {
                    return Err("Unexpected WS message type".to_owned())
                        .map_err(logthru_net!(error));
                }
                None => {
                    return Err("WS stream closed".to_owned()).map_err(logthru_net!(error));
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

pub struct WebsocketProtocolHandler {}

impl WebsocketProtocolHandler {
    pub async fn connect(
        network_manager: NetworkManager,
        dial_info: &DialInfo,
    ) -> Result<NetworkConnection, String> {
        let url = dial_info
            .request()
            .ok_or_else(|| format!("missing url in websocket dialinfo: {:?}", dial_info))?;
        let split_url = SplitUrl::from_str(&url)?;
        let tls = match dial_info {
            DialInfo::WS(ws) => {
                if split_url.scheme.to_ascii_lowercase() != "ws" {
                    return Err(format!("wrong scheme for WS websocket url: {}", url));
                }
                false
            }
            DialInfo::WSS(wss) => {
                if split_url.scheme.to_ascii_lowercase() != "wss" {
                    return Err(format!("wrong scheme for WSS websocket url: {}", url));
                }
                true
            }
            _ => {
                return Err("wrong protocol for WebsocketProtocolHandler".to_owned())
                    .map_err(logthru_net!(error))
            }
        };
        let peer_addr = dial_info.to_peer_address();

        let (ws, wsio) = WsMeta::connect(url, None)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error))?;

        let conn = NetworkConnection::WS(WebsocketNetworkConnection::new(tls, ws, wsio));
        network_manager
            .on_new_connection(ConnectionDescriptor::new_no_local(peer_addr), conn.clone())
            .await?;

        Ok(conn)
    }
}
