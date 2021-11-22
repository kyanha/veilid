use crate::command_processor::*;
use crate::veilid_client_capnp::*;
use anyhow::*;
use async_std::prelude::*;
use capnp::capability::Promise;
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, Disconnector, RpcSystem};
use futures::AsyncReadExt;
use log::*;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;

struct VeilidClientImpl {
    comproc: CommandProcessor,
}

impl VeilidClientImpl {
    pub fn new(comproc: CommandProcessor) -> Self {
        Self { comproc: comproc }
    }
}

impl veilid_client::Server for VeilidClientImpl {
    fn state_changed(
        &mut self,
        params: veilid_client::StateChangedParams,
        _results: veilid_client::StateChangedResults,
    ) -> Promise<(), ::capnp::Error> {
        let changed = pry!(pry!(params.get()).get_changed());

        if changed.has_attachment() {
            let attachment = pry!(changed.get_attachment());
            let old_state = pry!(attachment.get_old_state());
            let new_state = pry!(attachment.get_new_state());

            trace!(
                "AttachmentStateChange: old_state={} new_state={}",
                old_state as u16,
                new_state as u16
            );
            self.comproc.set_attachment_state(new_state);
        }

        Promise::ok(())
    }
}

struct ClientApiConnectionInner {
    comproc: CommandProcessor,
    connect_addr: Option<SocketAddr>,
    disconnector: Option<Disconnector<rpc_twoparty_capnp::Side>>,
    server: Option<Rc<RefCell<veilid_server::Client>>>,
    disconnect_requested: bool,
}

type Handle<T> = Rc<RefCell<T>>;

#[derive(Clone)]
pub struct ClientApiConnection {
    inner: Handle<ClientApiConnectionInner>,
}

impl ClientApiConnection {
    pub fn new(comproc: CommandProcessor) -> Self {
        Self {
            inner: Rc::new(RefCell::new(ClientApiConnectionInner {
                comproc: comproc,
                connect_addr: None,
                disconnector: None,
                server: None,
                disconnect_requested: false,
            })),
        }
    }

    async fn handle_connection(&mut self) -> Result<()> {
        trace!("ClientApiConnection::handle_connection");
        let connect_addr = self.inner.borrow().connect_addr.unwrap().clone();
        // Connect the TCP socket
        let stream = async_std::net::TcpStream::connect(connect_addr.clone()).await?;
        // If it succeed, disable nagle algorithm
        stream.set_nodelay(true)?;

        // Create the VAT network
        let (reader, writer) = stream.split();
        let rpc_network = Box::new(twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Client,
            Default::default(),
        ));
        // Create the rpc system
        let mut rpc_system = RpcSystem::new(rpc_network, None);
        let mut request;
        {
            let mut inner = self.inner.borrow_mut();

            // Get the bootstrap server connection object
            inner.server = Some(Rc::new(RefCell::new(
                rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server),
            )));

            // Store our disconnector future for later (must happen after bootstrap, contrary to documentation)
            inner.disconnector = Some(rpc_system.get_disconnector());

            // Get a client object to pass to the server for status update callbacks
            let client = capnp_rpc::new_client(VeilidClientImpl::new(inner.comproc.clone()));

            // Register our client and get a registration object back
            request = inner
                .server
                .as_ref()
                .unwrap()
                .borrow_mut()
                .register_request();
            request.get().set_veilid_client(client);

            inner
                .comproc
                .set_connection_state(ConnectionState::Connected(
                    connect_addr,
                    std::time::SystemTime::now(),
                ));
        }

        // Don't drop the registration
        rpc_system.try_join(request.send().promise).await?;

        // Drop the server and disconnector too (if we still have it)
        let mut inner = self.inner.borrow_mut();
        let disconnect_requested = inner.disconnect_requested;
        inner.server = None;
        inner.disconnector = None;
        inner.disconnect_requested = false;

        if !disconnect_requested {
            // Connection lost
            Err(anyhow!("Connection lost"))
        } else {
            // Connection finished
            Ok(())
        }
    }

    pub async fn server_attach(&mut self) -> Result<bool> {
        trace!("ClientApiConnection::server_attach");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or(anyhow!("Not connected, ignoring attach request"))?
                .clone()
        };
        let request = server.borrow().attach_request();
        let response = request.send().promise.await?;
        Ok(response.get()?.get_result())
    }

    pub async fn server_detach(&mut self) -> Result<bool> {
        trace!("ClientApiConnection::server_detach");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or(anyhow!("Not connected, ignoring detach request"))?
                .clone()
        };
        let request = server.borrow().detach_request();
        let response = request.send().promise.await?;
        Ok(response.get()?.get_result())
    }

    pub async fn server_shutdown(&mut self) -> Result<bool> {
        trace!("ClientApiConnection::server_shutdown");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or(anyhow!("Not connected, ignoring attach request"))?
                .clone()
        };
        let request = server.borrow().shutdown_request();
        let response = request.send().promise.await?;
        Ok(response.get()?.get_result())
    }

    // Start Client API connection
    pub async fn connect(&mut self, connect_addr: SocketAddr) -> Result<()> {
        trace!("ClientApiConnection::connect");
        // Save the address to connect to
        self.inner.borrow_mut().connect_addr = Some(connect_addr);

        self.handle_connection().await
    }

    // End Client API connection
    pub async fn disconnect(&mut self) {
        trace!("ClientApiConnection::disconnect");
        let disconnector = self.inner.borrow_mut().disconnector.take();
        match disconnector {
            Some(d) => {
                self.inner.borrow_mut().disconnect_requested = true;
                d.await.unwrap();
                self.inner.borrow_mut().connect_addr = None;
            }
            None => {
                debug!("disconnector doesn't exist");
            }
        }
    }
}
