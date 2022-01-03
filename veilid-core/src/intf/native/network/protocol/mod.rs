pub mod tcp;
pub mod udp;
pub mod wrtc;
pub mod ws;

use crate::xx::*;
use crate::*;
use socket2::{Domain, Protocol, Socket, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DummyNetworkConnection {}

impl DummyNetworkConnection {
    pub fn connection_descriptor(&self) -> ConnectionDescriptor {
        ConnectionDescriptor::new_no_local(PeerAddress::new(
            SocketAddress::default(),
            ProtocolType::UDP,
        ))
    }
    pub async fn send(&self, _message: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
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
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<NetworkConnection, String> {
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

    pub fn connection_descriptor(&self) -> ConnectionDescriptor {
        match self {
            Self::Dummy(d) => d.connection_descriptor(),
            Self::RawTcp(t) => t.connection_descriptor(),
            Self::WsAccepted(w) => w.connection_descriptor(),
            Self::Ws(w) => w.connection_descriptor(),
            Self::Wss(w) => w.connection_descriptor(),
        }
    }
    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.send(message).await,
            Self::RawTcp(t) => t.send(message).await,
            Self::WsAccepted(w) => w.send(message).await,
            Self::Ws(w) => w.send(message).await,
            Self::Wss(w) => w.send(message).await,
        }
    }
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Dummy(d) => d.recv().await,
            Self::RawTcp(t) => t.recv().await,
            Self::WsAccepted(w) => w.recv().await,
            Self::Ws(w) => w.recv().await,
            Self::Wss(w) => w.recv().await,
        }
    }
}

pub fn new_unbound_shared_udp_socket(domain: Domain) -> Result<socket2::Socket, String> {
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
    Ok(socket)
}

pub fn new_bound_shared_udp_socket(local_address: SocketAddr) -> Result<socket2::Socket, String> {
    let domain = Domain::for_address(local_address);
    let socket = new_unbound_shared_udp_socket(domain)?;
    let socket2_addr = socket2::SockAddr::from(local_address);
    socket
        .bind(&socket2_addr)
        .map_err(|e| format!("failed to bind UDP socket: {}", e))?;

    log_net!("created shared udp socket on {:?}", &local_address);

    Ok(socket)
}

pub fn new_unbound_shared_tcp_socket(domain: Domain) -> Result<socket2::Socket, String> {
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(map_to_string)
        .map_err(logthru_net!("failed to create TCP socket"))?;
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
    Ok(socket)
}

pub fn new_bound_shared_tcp_socket(local_address: SocketAddr) -> Result<socket2::Socket, String> {
    let domain = Domain::for_address(local_address);

    let socket = new_unbound_shared_tcp_socket(domain)?;

    let socket2_addr = socket2::SockAddr::from(local_address);
    socket
        .bind(&socket2_addr)
        .map_err(|e| format!("failed to bind TCP socket: {}", e))?;

    Ok(socket)
}
