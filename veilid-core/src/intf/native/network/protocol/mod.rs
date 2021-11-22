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
    WSAccepted(ws::WebSocketNetworkConnectionAccepted),
    WS(ws::WebsocketNetworkConnectionWS),
    WSS(ws::WebsocketNetworkConnectionWSS),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl NetworkConnection {
    pub fn protocol_type(&self) -> ProtocolType {
        match self {
            Self::Dummy(d) => d.protocol_type(),
            Self::RawTcp(t) => t.protocol_type(),
            Self::WSAccepted(w) => w.protocol_type(),
            Self::WS(w) => w.protocol_type(),
            Self::WSS(w) => w.protocol_type(),
        }
    }
    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), ()>> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::RawTcp(t) => t.send(message),
            Self::WSAccepted(w) => w.send(message),
            Self::WS(w) => w.send(message),
            Self::WSS(w) => w.send(message),
        }
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, ()>> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::RawTcp(t) => t.recv(),
            Self::WSAccepted(w) => w.recv(),
            Self::WS(w) => w.recv(),
            Self::WSS(w) => w.recv(),
        }
    }
}
