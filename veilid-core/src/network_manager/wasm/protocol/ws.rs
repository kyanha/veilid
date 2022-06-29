use super::*;
use ws_stream_wasm::*;
use futures_util::{StreamExt, SinkExt};

struct WebsocketNetworkConnectionInner {
    ws_meta: WsMeta,
    ws_stream: CloneStream<WsStream>,
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
    pub fn new(
        descriptor: ConnectionDescriptor,
        ws_meta: WsMeta, ws_stream: WsStream) -> Self {
        Self {
            descriptor,
            inner: Arc::new(WebsocketNetworkConnectionInner {
                ws_meta,
                ws_stream: CloneStream::new(ws_stream),
            }),
        }
    }

    pub fn descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }

    #[instrument(level = "trace", err, skip(self))]
    pub async fn close(&self) -> Result<(), String> {
        self.inner.ws_meta.close().await.map_err(map_to_string).map(drop)
    }

    #[instrument(level = "trace", err, skip(self, message), fields(message.len = message.len()))]
    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        if message.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large WS message".to_owned()).map_err(logthru_net!(error));
        }
        self.inner.ws_stream.clone()
            .send(WsMessage::Binary(message)).await
            .map_err(|_| "failed to send to websocket".to_owned())
            .map_err(logthru_net!(error))
    }

    #[instrument(level = "trace", err, skip(self), fields(ret.len))]
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        let out = match self.inner.ws_stream.clone().next().await {
            Some(WsMessage::Binary(v)) => v,
            Some(x) => {
                return Err(format!("Unexpected WS message type: {:?}", x));
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
    #[instrument(level = "trace", err)]
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ProtocolNetworkConnection, String> {
        
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
        Ok(ProtocolNetworkConnection::Ws(WebsocketNetworkConnection::new(ConnectionDescriptor::new_no_local(
            dial_info.to_peer_address(),
        ), wsmeta, wsio)))
    }

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len()))]
    pub async fn send_unbound_message(dial_info: DialInfo, data: Vec<u8>) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound WS message".to_owned());
        }
        
        // Make the real connection
        let conn = Self::connect(None, dial_info)
            .await
            .map_err(|e| format!("failed to connect websocket for unbound message: {}", e))?;

        conn.send(data).await   
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

        let conn = Self::connect(None, dial_info.clone())
            .await
            .map_err(|e| format!("failed to connect websocket for unbound message: {}", e))?;

        conn.send(data).await?;
        let out = timeout(timeout_ms, conn.recv())
            .await
            .map_err(map_to_string)??;

        tracing::Span::current().record("ret.len", &out.len());
        Ok(out)
    }


}
