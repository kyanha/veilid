use crate::intf::*;
use crate::xx::*;
use crate::*;

///////////////////////////////////////////////////////////
// Accept

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use async_std::net::*;
        use utils::async_peek_stream::*;

        pub trait ProtocolAcceptHandler: ProtocolAcceptHandlerClone + Send + Sync {
            fn on_accept(
                &self,
                stream: AsyncPeekStream,
                peer_addr: SocketAddr,
            ) -> SystemPinBoxFuture<Result<Option<NetworkConnection>, String>>;
        }

        pub trait ProtocolAcceptHandlerClone {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler>;
        }

        impl<T> ProtocolAcceptHandlerClone for T
        where
            T: 'static + ProtocolAcceptHandler + Clone,
        {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler> {
                Box::new(self.clone())
            }
        }
        impl Clone for Box<dyn ProtocolAcceptHandler> {
            fn clone(&self) -> Box<dyn ProtocolAcceptHandler> {
                self.clone_box()
            }
        }

        pub type NewProtocolAcceptHandler =
            dyn Fn(VeilidConfig, bool, SocketAddr) -> Box<dyn ProtocolAcceptHandler> + Send;
    }
}

///////////////////////////////////////////////////////////
// Dummy protocol network connection for testing

#[derive(Debug)]
pub struct DummyNetworkConnection {}

impl DummyNetworkConnection {
    pub fn close(&self) -> Result<(), String> {
        Ok(())
    }
    pub fn send(&self, _message: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
    pub fn recv(&self) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }
}

///////////////////////////////////////////////////////////
// Top-level protocol independent network connection object

#[derive(Debug)]
struct NetworkConnectionInner {
    protocol_connection: ProtocolNetworkConnection,
    last_message_sent_time: Option<u64>,
    last_message_recv_time: Option<u64>,
}

#[derive(Debug)]
struct NetworkConnectionArc {
    descriptor: ConnectionDescriptor,
    established_time: u64,
    inner: AsyncMutex<NetworkConnectionInner>,
}

#[derive(Clone, Debug)]
pub struct NetworkConnection {
    arc: Arc<NetworkConnectionArc>,
}
impl PartialEq for NetworkConnection {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.arc) == Arc::as_ptr(&other.arc)
    }
}

impl Eq for NetworkConnection {}

impl NetworkConnection {
    fn new_inner(protocol_connection: ProtocolNetworkConnection) -> NetworkConnectionInner {
        NetworkConnectionInner {
            protocol_connection,
            last_message_sent_time: None,
            last_message_recv_time: None,
        }
    }
    fn new_arc(
        descriptor: ConnectionDescriptor,
        protocol_connection: ProtocolNetworkConnection,
    ) -> NetworkConnectionArc {
        NetworkConnectionArc {
            descriptor,
            established_time: intf::get_timestamp(),
            inner: AsyncMutex::new(Self::new_inner(protocol_connection)),
        }
    }

    pub fn dummy(descriptor: ConnectionDescriptor) -> Self {
        NetworkConnection::from_protocol(
            descriptor,
            ProtocolNetworkConnection::Dummy(DummyNetworkConnection {}),
        )
    }

    pub fn from_protocol(
        descriptor: ConnectionDescriptor,
        protocol_connection: ProtocolNetworkConnection,
    ) -> Self {
        Self {
            arc: Arc::new(Self::new_arc(descriptor, protocol_connection)),
        }
    }

    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<NetworkConnection, String> {
        ProtocolNetworkConnection::connect(local_address, dial_info).await
    }

    pub fn connection_descriptor(&self) -> ConnectionDescriptor {
        self.arc.descriptor
    }

    pub async fn close(&self) -> Result<(), String> {
        let mut inner = self.arc.inner.lock().await;
        inner.protocol_connection.close().await
    }

    pub async fn send(&self, message: Vec<u8>) -> Result<(), String> {
        let mut inner = self.arc.inner.lock().await;
        let out = inner.protocol_connection.send(message).await;
        if out.is_ok() {
            inner.last_message_sent_time = Some(intf::get_timestamp());
        }
        out
    }
    pub async fn recv(&self) -> Result<Vec<u8>, String> {
        let mut inner = self.arc.inner.lock().await;
        let out = inner.protocol_connection.recv().await;
        if out.is_ok() {
            inner.last_message_recv_time = Some(intf::get_timestamp());
        }
        out
    }
}
