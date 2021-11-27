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

    pub async fn on_message(&self, data: &[u8], remote_addr: SocketAddr) -> Result<bool, ()> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(());
        }

        trace!(
            "receiving message of length {} from {}",
            data.len(),
            remote_addr
        );

        // Process envelope
        let (network_manager, socket) = {
            let inner = self.inner.lock();
            (inner.network_manager.clone(), inner.socket.clone())
        };

        let peer_addr = PeerAddress::new(
            Address::from_socket_addr(remote_addr),
            remote_addr.port(),
            ProtocolType::UDP,
        );
        let local_socket_addr = socket.local_addr().map_err(drop)?;
        network_manager
            .on_recv_envelope(
                data,
                &ConnectionDescriptor::new(peer_addr, local_socket_addr),
            )
            .await
    }

    pub async fn send_message(&self, data: Vec<u8>, socket_addr: SocketAddr) -> Result<(), ()> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(());
        }

        trace!(
            "sending message of length {} to {}",
            data.len(),
            socket_addr
        );

        let socket = self.inner.lock().socket.clone();
        let len = socket.send_to(&data, socket_addr).await.map_err(drop)?;
        if len != data.len() {
            Err(())
        } else {
            Ok(())
        }
    }

    pub async fn send_unbound_message(data: Vec<u8>, socket_addr: SocketAddr) -> Result<(), ()> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(());
        }
        trace!(
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
        let socket = UdpSocket::bind(local_socket_addr).await.map_err(drop)?;
        let len = socket.send_to(&data, socket_addr).await.map_err(drop)?;
        if len != data.len() {
            Err(())
        } else {
            Ok(())
        }
    }
}
