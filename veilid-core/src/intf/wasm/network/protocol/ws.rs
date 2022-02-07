use crate::intf::*;
use crate::network_connection::*;
use crate::network_manager::MAX_MESSAGE_SIZE;
use crate::*;
use alloc::fmt;
use ws_stream_wasm::*;
use futures_util::{StreamExt, SinkExt};

struct WebsocketNetworkConnectionInner {
    ws_meta: WsMeta,
    ws_stream: CloneStream<WsStream>,
}

#[derive(Clone)]
pub struct WebsocketNetworkConnection {
    inner: Arc<WebsocketNetworkConnectionInner>,
}

impl fmt::Debug for WebsocketNetworkConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", core::any::type_name::<Self>())
    }
}

impl WebsocketNetworkConnection {
    pub fn new(ws_meta: WsMeta, ws_stream: WsStream) -> Self {
        Self {
            inner: Arc::new(WebsocketNetworkConnectionInner {
                ws_meta,
                ws_stream: CloneStream::new(ws_stream),
            }),
        }
    }

    pub async fn close(&self) -> Result<(), String> {
        self.inner.ws_meta.close().await.map_err(map_to_string).map(drop)
    }

    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        if message.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large WS message".to_owned()).map_err(logthru_net!(error));
        }
        self.inner.ws_stream.clone()
            .send(WsMessage::Binary(message)).await
            .map_err(|_| "failed to send to websocket".to_owned())
            .map_err(logthru_net!(error))
    }
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        let out = match self.inner.ws_stream.clone().next().await {
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
        dial_info: DialInfo,
    ) -> Result<NetworkConnection, String> {
        
        assert!(local_address.is_none());

        // Split dial info up
        let (_tls, scheme) = match &dial_info {
            DialInfo::WS(_) => (false, "ws"),
            DialInfo::WSS(_) => (true, "wss"),
            _ => panic!("invalid dialinfo for WS/WSS protocol"),
        };
        let request = dial_info.request().unwrap();
        let split_url = SplitUrl::from_str(&request)?;
        if split_url.scheme != scheme {
            return Err("invalid websocket url scheme".to_string());
        }
    
        let (wsmeta, wsio) = WsMeta::connect(request, None)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error))?;

        // Make our connection descriptor

        Ok(NetworkConnection::from_protocol(ConnectionDescriptor {
            local: None,
            remote: dial_info.to_peer_address(),
        },ProtocolNetworkConnection::Ws(WebsocketNetworkConnection::new(wsmeta, wsio))))
    }

    pub async fn send_unbound_message(dial_info: DialInfo, data: Vec<u8>) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound WS message".to_owned());
        }
        trace!(
            "sending unbound websocket message of length {} to {}",
            data.len(),
            dial_info,
        );

        let conn = Self::connect(None, dial_info)
            .await
            .map_err(|e| format!("failed to connect websocket for unbound message: {}", e))?;

        conn.send(data).await
    }
}
