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

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len()))]
    pub async fn send_unbound_message(socket_addr: SocketAddr, data: Vec<u8>) -> io::Result<()> {
        if data.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large unbound UDP message");
        }

        // get local wildcard address for bind
        let local_socket_addr = match socket_addr {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            SocketAddr::V6(_) => {
                SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0)
            }
        };
        let socket = UdpSocket::bind(local_socket_addr).await?;
        let len = socket.send_to(&data, socket_addr).await?;
        if len != data.len() {
            bail_io_error_other!("UDP partial unbound send")
        }

        Ok(())
    }

    #[instrument(level = "trace", err, skip(data), fields(data.len = data.len(), ret.len))]
    pub async fn send_recv_unbound_message(
        socket_addr: SocketAddr,
        data: Vec<u8>,
        timeout_ms: u32,
    ) -> io::Result<Vec<u8>> {
        if data.len() > MAX_MESSAGE_SIZE {
            bail_io_error_other!("sending too large unbound UDP message");
        }

        // get local wildcard address for bind
        let local_socket_addr = match socket_addr {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            SocketAddr::V6(_) => {
                SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0)
            }
        };

        // get unspecified bound socket
        let socket = UdpSocket::bind(local_socket_addr).await?;
        let len = socket.send_to(&data, socket_addr).await?;
        if len != data.len() {
            bail_io_error_other!("UDP partial unbound send");
        }

        // receive single response
        let mut out = vec![0u8; MAX_MESSAGE_SIZE];
        let (len, from_addr) = timeout(timeout_ms, socket.recv_from(&mut out))
            .await
            .map_err(|e| e.to_io())??;

        // if the from address is not the same as the one we sent to, then drop this
        if from_addr != socket_addr {
            bail_io_error_other!(format!(
                "Unbound response received from wrong address: addr={}",
                from_addr,
            ));
        }
        out.resize(len, 0u8);
        tracing::Span::current().record("ret.len", &len);
        Ok(out)
    }
}
