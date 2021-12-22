use crate::intf::*;
use crate::network_manager::{NetworkManager, MAX_MESSAGE_SIZE};
use crate::*;
use async_std::net::*;

struct RawUdpProtocolHandlerInner {
    network_manager: NetworkManager,
    socket: Arc<UdpSocket>,
}

#[derive(Clone)]
pub struct RawUdpProtocolHandler {
    inner: Arc<Mutex<RawUdpProtocolHandlerInner>>,
}

impl RawUdpProtocolHandler {
    fn new_inner(
        network_manager: NetworkManager,
        socket: Arc<UdpSocket>,
    ) -> RawUdpProtocolHandlerInner {
        RawUdpProtocolHandlerInner {
            network_manager,
            socket,
        }
    }

    pub fn new(network_manager: NetworkManager, socket: Arc<UdpSocket>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager, socket))),
        }
    }

    pub async fn on_message(&self, data: &[u8], remote_addr: SocketAddr) -> Result<bool, String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("received too large UDP message".to_owned());
        }

        trace!(
            "receiving UDP message of length {} from {}",
            data.len(),
            remote_addr
        );

        // Process envelope
        let (network_manager, socket) = {
            let inner = self.inner.lock();
            (inner.network_manager.clone(), inner.socket.clone())
        };

        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(remote_addr),
            ProtocolType::UDP,
        );
        let local_socket_addr = socket.local_addr().map_err(|e| format!("{}", e))?;
        network_manager
            .on_recv_envelope(
                data,
                &ConnectionDescriptor::new(
                    peer_addr,
                    SocketAddress::from_socket_addr(local_socket_addr),
                ),
            )
            .await
    }

    pub async fn send_message(&self, data: Vec<u8>, socket_addr: SocketAddr) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large UDP message".to_owned()).map_err(logthru_net!(error));
        }

        log_net!(
            "sending UDP message of length {} to {}",
            data.len(),
            socket_addr
        );

        let socket = self.inner.lock().socket.clone();
        let len = socket
            .send_to(&data, socket_addr)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed udp send: addr={}", socket_addr))?;

        if len != data.len() {
            Err("UDP partial send".to_owned()).map_err(logthru_net!(error))
        } else {
            Ok(())
        }
    }

    pub async fn send_unbound_message(
        data: Vec<u8>,
        socket_addr: SocketAddr,
    ) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large unbound UDP message".to_owned())
                .map_err(logthru_net!(error));
        }
        log_net!(
            "sending unbound message of length {} to {}",
            data.len(),
            socket_addr
        );

        // get local wildcard address for bind
        let local_socket_addr = match socket_addr {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            SocketAddr::V6(_) => {
                SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0)
            }
        };
        let socket = UdpSocket::bind(local_socket_addr)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed to bind unbound udp socket"))?;
        let len = socket
            .send_to(&data, socket_addr)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed unbound udp send: addr={}", socket_addr))?;
        if len != data.len() {
            Err("UDP partial unbound send".to_owned()).map_err(logthru_net!(error))
        } else {
            Ok(())
        }
    }
}
