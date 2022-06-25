pub mod sockets;
pub mod tcp;
pub mod udp;
pub mod wrtc;
pub mod ws;

use super::*;
use crate::xx::*;

#[derive(Debug)]
pub enum ProtocolNetworkConnection {
    Dummy(DummyNetworkConnection),
    RawTcp(tcp::RawTcpNetworkConnection),
    WsAccepted(ws::WebSocketNetworkConnectionAccepted),
    Ws(ws::WebsocketNetworkConnectionWS),
    Wss(ws::WebsocketNetworkConnectionWSS),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl ProtocolNetworkConnection {
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ProtocolNetworkConnection, String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("Should not connect to UDP dialinfo");
            }
            ProtocolType::TCP => {
                tcp::RawTcpProtocolHandler::connect(local_address, dial_info).await
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::connect(local_address, dial_info).await
            }
        }
    }

    pub async fn send_unbound_message(dial_info: DialInfo, data: Vec<u8>) -> Result<(), String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                udp::RawUdpProtocolHandler::send_unbound_message(peer_socket_addr, data).await
            }
            ProtocolType::TCP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                tcp::RawTcpProtocolHandler::send_unbound_message(peer_socket_addr, data).await
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
                let peer_socket_addr = dial_info.to_socket_addr();
                udp::RawUdpProtocolHandler::send_recv_unbound_message(
                    peer_socket_addr,
                    data,
                    timeout_ms,
                )
                .await
            }
            ProtocolType::TCP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                tcp::RawTcpProtocolHandler::send_recv_unbound_message(
                    peer_socket_addr,
                    data,
                    timeout_ms,
                )
                .await
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
            Self::RawTcp(t) => t.descriptor(),
            Self::WsAccepted(w) => w.descriptor(),
            Self::Ws(w) => w.descriptor(),
            Self::Wss(w) => w.descriptor(),
        }
    }

    pub async fn close(&self) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.close(),
            Self::RawTcp(t) => t.close().await,
            Self::WsAccepted(w) => w.close().await,
            Self::Ws(w) => w.close().await,
            Self::Wss(w) => w.close().await,
        }
    }

    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::RawTcp(t) => t.send(message).await,
            Self::WsAccepted(w) => w.send(message).await,
            Self::Ws(w) => w.send(message).await,
            Self::Wss(w) => w.send(message).await,
        }
    }
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::RawTcp(t) => t.recv().await,
            Self::WsAccepted(w) => w.recv().await,
            Self::Ws(w) => w.recv().await,
            Self::Wss(w) => w.recv().await,
        }
    }
}
