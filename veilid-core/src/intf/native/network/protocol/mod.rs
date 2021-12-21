pub mod tcp;
pub mod udp;
pub mod wrtc;
pub mod ws;

use super::listener_state::*;
use crate::veilid_api::ProtocolType;
use crate::xx::*;
use socket2::{Domain, Protocol, Socket, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DummyNetworkConnection {}

impl DummyNetworkConnection {
    pub fn protocol_type(&self) -> ProtocolType {
        ProtocolType::UDP
    }
    pub fn send(&self, _message: Vec<u8>) -> SystemPinBoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, String>> {
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
    pub fn send(&self, message: Vec<u8>) -> SystemPinBoxFuture<Result<(), String>> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::RawTcp(t) => t.send(message),
            Self::WsAccepted(w) => w.send(message),
            Self::Ws(w) => w.send(message),
            Self::Wss(w) => w.send(message),
        }
    }
    pub fn recv(&self) -> SystemPinBoxFuture<Result<Vec<u8>, String>> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::RawTcp(t) => t.recv(),
            Self::WsAccepted(w) => w.recv(),
            Self::Ws(w) => w.recv(),
            Self::Wss(w) => w.recv(),
        }
    }
}

pub fn new_shared_udp_socket(local_address: SocketAddr) -> Result<socket2::Socket, String> {
    let domain = Domain::for_address(local_address);
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| format!("Couldn't create UDP socket: {}", e))?;

    if let Err(e) = socket.set_reuse_address(true) {
        log_net!(error "Couldn't set reuse address: {}", e);
    }
    cfg_if! {
        if #[cfg(unix)] {
            if let Err(e) = socket.set_reuse_port(true) {
                log_net!(error "Couldn't set reuse port: {}", e);
            }
        }
    }

    let socket2_addr = socket2::SockAddr::from(local_address);
    socket
        .bind(&socket2_addr)
        .map_err(|e| format!("failed to bind UDP socket: {}", e))?;

    Ok(socket)
}

pub fn new_shared_tcp_socket(local_address: SocketAddr) -> Result<socket2::Socket, String> {
    let domain = Domain::for_address(local_address);
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(map_to_string)
        .map_err(logthru_net!())?;
    if let Err(e) = socket.set_linger(None) {
        log_net!(error "Couldn't set TCP linger: {}", e);
    }
    if let Err(e) = socket.set_nodelay(true) {
        log_net!(error "Couldn't set TCP nodelay: {}", e);
    }
    if let Err(e) = socket.set_reuse_address(true) {
        log_net!(error "Couldn't set reuse address: {}", e);
    }
    cfg_if! {
        if #[cfg(unix)] {
            if let Err(e) = socket.set_reuse_port(true) {
                log_net!(error "Couldn't set reuse port: {}", e);
            }
        }
    }

    let socket2_addr = socket2::SockAddr::from(local_address);
    if let Err(e) = socket.bind(&socket2_addr) {
        log_net!(error "failed to bind TCP socket: {}", e);
    }

    Ok(socket)
}
