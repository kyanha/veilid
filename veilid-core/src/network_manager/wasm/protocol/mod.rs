pub mod wrtc;
pub mod ws;

use super::*;
use crate::xx::*;

#[derive(Debug)]
pub enum ProtocolNetworkConnection {
    Dummy(DummyNetworkConnection),
    Ws(ws::WebsocketNetworkConnection),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl ProtocolNetworkConnection {
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ProtocolNetworkConnection, String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("UDP dial info is not supported on WASM targets");
            }
            ProtocolType::TCP => {
                panic!("TCP dial info is not supported on WASM targets");
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::connect(local_address, dial_info).await
            }
        }
    }

    pub async fn send_unbound_message(
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("UDP dial info is not supported on WASM targets");
            }
            ProtocolType::TCP => {
                panic!("TCP dial info is not supported on WASM targets");
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::send_unbound_message(dial_info, data).await
            }
        }
    }

    pub async fn send_recv_unbound_message(
        dial_info: DialInfo,
        data: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<Vec<u8>, String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("UDP dial info is not supported on WASM targets");
            }
            ProtocolType::TCP => {
                panic!("TCP dial info is not supported on WASM targets");
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::send_recv_unbound_message(dial_info, data, timeout_ms)
                    .await
            }
        }
    }

    pub fn descriptor(&self) -> ConnectionDescriptor {
        match self {
            Self::Dummy(d) => d.descriptor(),
            Self::Ws(w) => w.descriptor(),
        }
    }

    pub async fn close(&self) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.close(),
            Self::Ws(w) => w.close().await,
        }
    }
    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::Ws(w) => w.send(message).await,
        }
    }

    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::Ws(w) => w.recv().await,
        }
    }
}
