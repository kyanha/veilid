use super::*;

#[derive(Clone)]
pub struct RawUdpProtocolHandler {
    socket: Arc<UdpSocket>,
}

impl RawUdpProtocolHandler {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        Self { socket }
    }

    #[instrument(level = "trace", err, skip(self, data), fields(data.len = data.len(), ret.len, ret.from))]
    pub async fn recv_message(
        &self,
        data: &mut [u8],
    ) -> Result<(usize, ConnectionDescriptor), String> {
        let (size, remote_addr) = self.socket.recv_from(data).await.map_err(map_to_string)?;

        if size > MAX_MESSAGE_SIZE {
            return Err("received too large UDP message".to_owned());
        }

        trace!(
            "receiving UDP message of length {} from {}",
            size,
            remote_addr
        );

        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(remote_addr),
            ProtocolType::UDP,
        );
        let local_socket_addr = self.socket.local_addr().map_err(map_to_string)?;
        let descriptor = ConnectionDescriptor::new(
            peer_addr,
            SocketAddress::from_socket_addr(local_socket_addr),
        );

        tracing::Span::current().record("ret.len", &size);
        tracing::Span::current().record("ret.from", &format!("{:?}", descriptor).as_str());
        Ok((size, descriptor))
    }

    #[instrument(level = "trace", err, skip(self, data), fields(data.len = data.len(), ret.len, ret.from))]
    pub async fn send_message(&self, data: Vec<u8>, socket_addr: SocketAddr) -> Result<(), String> {
        if data.len() > MAX_MESSAGE_SIZE {
            return Err("sending too large UDP message".to_owned()).map_err(logthru_net!(error));
        }

        log_net!(
            "sending UDP message of length {} to {}",
            data.len(),
            socket_addr
        );

        let len = self
            .socket
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
        socket_addr: SocketAddr,
        data: Vec<u8>,
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
