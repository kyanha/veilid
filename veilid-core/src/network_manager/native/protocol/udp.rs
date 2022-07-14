use super::*;
use sockets::*;

#[derive(Clone)]
pub struct RawUdpProtocolHandler {
    socket: Arc<UdpSocket>,
}

impl RawUdpProtocolHandler {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        Self { socket }
    }

    #[instrument(level = "trace", err, skip(self, data), fields(data.len = data.len(), ret.len, ret.from))]
    pub async fn recv_message(&self, data: &mut [u8]) -> io::Result<(usize, ConnectionDescriptor)> {
        let (size, remote_addr) = loop {
            match self.socket.recv_from(data).await {
                Ok((size, remote_addr)) => {
                    if size > MAX_MESSAGE_SIZE {
                        bail_io_error_other!("received too large UDP message");
                    }
                    break (size, remote_addr);
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::ConnectionReset {
                        // Ignore icmp
                    } else {
                        return Err(e);
                    }
                }
            }
        };

        let peer_addr = PeerAddress::new(
            SocketAddress::from_socket_addr(remote_addr),
            ProtocolType::UDP,
        );
        let local_socket_addr = self.socket.local_addr()?;
        let descriptor = ConnectionDescriptor::new(
            peer_addr,
            SocketAddress::from_socket_addr(local_socket_addr),
        );

        tracing::Span::current().record("ret.len", &size);
        tracing::Span::current().record("ret.from", &format!("{:?}", descriptor).as_str());
        Ok((size, descriptor))
    }

    #[instrument(level = "trace", err, skip(self, data), fields(data.len = data.len(), ret.len, ret.from))]
    pub async fn send_message(&self, data: Vec<u8>, socket_addr: SocketAddr) -> io::Result<()> {
        if data.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large UDP message");
        }

        let len = self.socket.send_to(&data, socket_addr).await?;
        if len != data.len() {
            bail_io_error_other!("UDP partial send")
        }

        Ok(())
    }

    #[instrument(level = "trace", err)]
    pub async fn new_unspecified_bound_handler(
        socket_addr: &SocketAddr,
    ) -> io::Result<RawUdpProtocolHandler> {
        // get local wildcard address for bind
        let local_socket_addr = compatible_unspecified_socket_addr(&socket_addr);
        let socket = UdpSocket::bind(local_socket_addr).await?;
        Ok(RawUdpProtocolHandler::new(Arc::new(socket)))
    }
}
