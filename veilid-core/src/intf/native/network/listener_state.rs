use crate::intf::*;
use crate::network_manager::*;
use utils::async_peek_stream::*;

use async_std::net::*;
use async_tls::TlsAcceptor;

pub trait TcpProtocolHandler: TcpProtocolHandlerClone + Send + Sync {
    fn on_accept(
        &self,
        stream: AsyncPeekStream,
        peer_addr: SocketAddr,
    ) -> SendPinBoxFuture<Result<bool, ()>>;
}

pub trait TcpProtocolHandlerClone {
    fn clone_box(&self) -> Box<dyn TcpProtocolHandler>;
}

impl<T> TcpProtocolHandlerClone for T
where
    T: 'static + TcpProtocolHandler + Clone,
{
    fn clone_box(&self) -> Box<dyn TcpProtocolHandler> {
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn TcpProtocolHandler> {
    fn clone(&self) -> Box<dyn TcpProtocolHandler> {
        self.clone_box()
    }
}

pub type NewTcpProtocolHandler =
    dyn Fn(NetworkManager, bool, SocketAddr) -> Box<dyn TcpProtocolHandler> + Send;

/////////////////////////////////////////////////////////////////

#[derive(Clone)]
pub struct ListenerState {
    pub protocol_handlers: Vec<Box<dyn TcpProtocolHandler + 'static>>,
    pub tls_protocol_handlers: Vec<Box<dyn TcpProtocolHandler + 'static>>,
    pub tls_acceptor: Option<TlsAcceptor>,
}

impl ListenerState {
    pub fn new() -> Self {
        Self {
            protocol_handlers: Vec::new(),
            tls_protocol_handlers: Vec::new(),
            tls_acceptor: None,
        }
    }
}
