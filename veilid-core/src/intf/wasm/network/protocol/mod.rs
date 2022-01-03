pub mod wrtc;
pub mod ws;

use crate::connection_manager::*;
use crate::veilid_api::ProtocolType;
use crate::xx::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkConnection {
    Dummy(DummyNetworkConnection),
    WS(ws::WebsocketNetworkConnection),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl NetworkConnection {
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<NetworkConnection, String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("UDP dial info is not support on WASM targets");
            }
            ProtocolType::TCP => {
                panic!("TCP dial info is not support on WASM targets");
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::connect(local_address, dial_info).await
            }
        }
    }

    pub async fn send_unbound_message(
        &self,
        dial_info: &DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("UDP dial info is not support on WASM targets");
            }
            ProtocolType::TCP => {
                panic!("TCP dial info is not support on WASM targets");
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::send_unbound_message(dial_info, data).await
            }
        }
    }

    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.send(message).await,
            Self::WS(w) => w.send(message).await,
        }
    }

    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Dummy(d) => d.recv().await,
            Self::WS(w) => w.recv().await,
        }
    }
}
