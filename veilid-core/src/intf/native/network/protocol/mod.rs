pub mod tcp;
pub mod udp;
pub mod wrtc;
pub mod ws;

use super::listener_state::*;
use crate::veilid_api::ProtocolType;
use crate::xx::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DummyNetworkConnection {}

impl DummyNetworkConnection {
    pub fn protocol_type(&self) -> ProtocolType {
        ProtocolType::UDP
    }
    pub fn send(&self, _message: Vec<u8>) -> SystemPinBoxFuture<Result<(), ()>> {
        Box::pin(async { Ok(()) })
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, ()>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkConnection {
    Dummy(DummyNetworkConnection),
    RawTcp(tcp::RawTcpNetworkConnection),
    WsAccepted(ws::WebSocketNetworkConnectionAccepted),
    Ws(ws::WebsocketNetworkConnectionWS),
    Wss(ws::WebsocketNetworkConnectionWSS),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl NetworkConnection {
    pub fn protocol_type(&self) -> ProtocolType {
        match self {
            Self::Dummy(d) => d.protocol_type(),
            Self::RawTcp(t) => t.protocol_type(),
            Self::WsAccepted(w) => w.protocol_type(),
            Self::Ws(w) => w.protocol_type(),
            Self::Wss(w) => w.protocol_type(),
        }
    }
    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), ()>> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::RawTcp(t) => t.send(message),
            Self::WsAccepted(w) => w.send(message),
            Self::Ws(w) => w.send(message),
            Self::Wss(w) => w.send(message),
        }
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, ()>> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::RawTcp(t) => t.recv(),
            Self::WsAccepted(w) => w.recv(),
            Self::Ws(w) => w.recv(),
            Self::Wss(w) => w.recv(),
        }
    }
}
