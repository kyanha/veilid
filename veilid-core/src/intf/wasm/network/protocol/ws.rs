use crate::intf::*;
use crate::network_manager::{NetworkManager, MAX_MESSAGE_SIZE};
use crate::*;
use alloc::fmt;
use futures_util::stream::StreamExt;
use web_sys::WebSocket;
use ws_stream_wasm::*;

struct WebsocketNetworkConnectionInner {
    ws_meta: WsMeta,
    ws_stream: WsStream,
}

#[derive(Clone)]
pub struct WebsocketNetworkConnection {
    tls: bool,
    inner: Arc<AsyncMutex<WebsocketNetworkConnectionInner>>,
}

impl fmt::Debug for WebsocketNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", core::any::type_name::<Self>())
    }
}

impl WebsocketNetworkConnection {
    pub fn new(tls: bool, ws_meta: WsMeta, ws_stream: WsStream) -> Self {
        Self {
            tls,
            inner: Arc::new(Mutex::new(WebsocketNetworkConnectionInner {
                ws_meta,
                ws_stream,
            })),
        }
    }

    pub async fn close(&self) -> Result<(), String> {
        let inner = self.inner.lock().await;
        inner.ws_meta.close().await;
    }

    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        if message.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large WS message".to_owned()).map_err(logthru_net!(error));
        }
        let mut inner = self.inner.lock().await;
        inner.ws_stream
            .send(WsMessage::Binary(message)).await
            .map_err(|_| "failed to send to websocket".to_owned())
            .map_err(logthru_net!(error))
    }
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        let mut inner = self.inner.lock().await;
        let out = match inner.ws_stream.next().await {
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
    }
}

///////////////////////////////////////////////////////////
///

pub struct WebsocketProtocolHandler {}

impl WebsocketProtocolHandler {
    pub async fn connect(
        local_address: Option<SocketAddr>,
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

        let (_, wsio) = WsMeta::connect(url, None)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error))?;

        // Make our connection descriptor
        let connection_descriptor = ConnectionDescriptor {
            local: None,
            remote: dial_info.to_peer_address(),
        };

        Ok(NetworkConnection::from_protocol(descriptor,ProtocolNetworkConnection::Ws(WebsocketNetworkConnection::new(tls, wsio))))
    }

    pub async fn send_unbound_message(dial_info: &DialInfo, data: Vec<u8>) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound WS message".to_owned());
        }
        trace!(
            "sending unbound websocket message of length {} to {}",
            data.len(),
            dial_info,
        );

        let conn = Self::connect(None, dial_info.clone())
            .await
            .map_err(|e| format!("failed to connect websocket for unbound message: {}", e))?;

        conn.send(data).await
    }
}
