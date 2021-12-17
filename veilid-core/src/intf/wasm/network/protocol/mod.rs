pub mod wrtc;
pub mod ws;

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
    WS(ws::WebsocketNetworkConnection),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl NetworkConnection {
    pub fn protocol_type(&self) -> ProtocolType {
        match self {
            Self::Dummy(d) => d.protocol_type(),
            Self::WS(w) => w.protocol_type(),
        }
    }
    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), String>> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::WS(w) => w.send(message),
        }
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, String>> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::WS(w) => w.recv(),
        }
    }
}
