pub mod tcp;
pub mod udp;
pub mod wrtc;
pub mod ws;

use crate::network_connection::*;
use crate::xx::*;
use crate::*;
use socket2::{Domain, Protocol, Socket, Type};

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

    pub async fn send_unbound_message(dial_info: &DialInfo, data: Vec<u8>) -> Result<(), String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                udp::RawUdpProtocolHandler::send_unbound_message(peer_socket_addr, data)
                    .await
                    .map_err(logthru_net!())
            }
            ProtocolType::TCP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                tcp::RawTcpProtocolHandler::send_unbound_message(peer_socket_addr, data)
                    .await
                    .map_err(logthru_net!())
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::send_unbound_message(dial_info, data).await
            }
        }
    }

    pub async fn close(&mut self) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.close(),
            Self::RawTcp(t) => t.close().await,
            Self::WsAccepted(w) => w.close().await,
            Self::Ws(w) => w.close().await,
            Self::Wss(w) => w.close().await,
        }
    }

    pub async fn send(&mut self, message: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::RawTcp(t) => t.send(message).await,
            Self::WsAccepted(w) => w.send(message).await,
            Self::Ws(w) => w.send(message).await,
            Self::Wss(w) => w.send(message).await,
        }
    }
    pub async fn recv(&mut self) -> Result<Vec<u8>, String> {
        match self {
            Self::Dummy(d) => d.recv(),
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
