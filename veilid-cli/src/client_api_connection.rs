use crate::command_processor::*;
use crate::veilid_client_capnp::*;
use veilid_core::xx::*;

use capnp::capability::Promise;
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, Disconnector, RpcSystem};
use futures::AsyncReadExt;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;

struct VeilidClientImpl {
    comproc: CommandProcessor,
}

impl VeilidClientImpl {
    pub fn new(comproc: CommandProcessor) -> Self {
        Self { comproc }
    }
}

impl veilid_client::Server for VeilidClientImpl {
    fn update(
        &mut self,
        params: veilid_client::UpdateParams,
        _results: veilid_client::UpdateResults,
    ) -> Promise<(), ::capnp::Error> {
        let veilid_update = pry!(pry!(params.get()).get_veilid_update());

        let which = match veilid_update.which() {
            Ok(v) => v,
            Err(e) => {
                panic!("(missing update kind in schema: {:?})", e);
            }
        };
        match which {
            veilid_update::Attachment(Ok(attachment)) => {
                let state = pry!(attachment.get_state());

                trace!("Attachment: {}", state as u16);
                self.comproc.update_attachment(state);
            }
            _ => {
                panic!("shouldn't get this")
            }
        }

        Promise::ok(())
    }

    fn log_message(
        &mut self,
        params: veilid_client::LogMessageParams,
        _results: veilid_client::LogMessageResults,
    ) -> Promise<(), ::capnp::Error> {
        let message = pry!(pry!(params.get()).get_message());
        self.comproc.add_log_message(message);
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
                comproc,
                connect_addr: None,
                disconnector: None,
                server: None,
                disconnect_requested: false,
            })),
        }
    }
    async fn process_veilid_state<'a>(
        &'a mut self,
        state: veilid_state::Reader<'a>,
    ) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();

        // Process attachment state
        let attachment = state.reborrow().get_attachment().map_err(map_to_string)?;
        let state = attachment.get_state().map_err(map_to_string)?;
        inner.comproc.update_attachment(state);

        Ok(())
    }

    async fn handle_connection(&mut self) -> Result<(), String> {
        trace!("ClientApiConnection::handle_connection");
        let connect_addr = self.inner.borrow().connect_addr.unwrap();
        // Connect the TCP socket
        let stream = async_std::net::TcpStream::connect(connect_addr)
            .await
            .map_err(map_to_string)?;
        // If it succeed, disable nagle algorithm
        stream.set_nodelay(true).map_err(map_to_string)?;

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

        // Send the request and get the state object and the registration object
        if let Ok(response) = request.send().promise.await {
            if let Ok(response) = response.get() {
                if let Ok(_registration) = response.get_registration() {
                    if let Ok(state) = response.get_state() {
                        // Set up our state for the first time
                        if self.process_veilid_state(state).await.is_ok() {
                            // Don't drop the registration
                            rpc_system.await.map_err(map_to_string)?;
                        }
                    }
                }
            }
        }

        // Drop the server and disconnector too (if we still have it)
        let mut inner = self.inner.borrow_mut();
        let disconnect_requested = inner.disconnect_requested;
        inner.server = None;
        inner.disconnector = None;
        inner.disconnect_requested = false;

        if !disconnect_requested {
            // Connection lost
            Err("Connection lost".to_owned())
        } else {
            // Connection finished
            Ok(())
        }
    }

    pub async fn server_attach(&mut self) -> Result<(), String> {
        trace!("ClientApiConnection::server_attach");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or_else(|| "Not connected, ignoring attach request".to_owned())?
                .clone()
        };
        let request = server.borrow().attach_request();
        let response = request.send().promise.await.map_err(map_to_string)?;
        response.get().map(drop).map_err(map_to_string)
    }

    pub async fn server_detach(&mut self) -> Result<(), String> {
        trace!("ClientApiConnection::server_detach");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or_else(|| "Not connected, ignoring detach request".to_owned())?
                .clone()
        };
        let request = server.borrow().detach_request();
        let response = request.send().promise.await.map_err(map_to_string)?;
        response.get().map(drop).map_err(map_to_string)
    }

    pub async fn server_shutdown(&mut self) -> Result<(), String> {
        trace!("ClientApiConnection::server_shutdown");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or_else(|| "Not connected, ignoring attach request".to_owned())?
                .clone()
        };
        let request = server.borrow().shutdown_request();
        let response = request.send().promise.await.map_err(map_to_string)?;
        response.get().map(drop).map_err(map_to_string)
    }

    pub async fn server_debug(&mut self, what: String) -> Result<String, String> {
        trace!("ClientApiConnection::server_debug");
        let server = {
            let inner = self.inner.borrow();
            inner
                .server
                .as_ref()
                .ok_or_else(|| "Not connected, ignoring attach request".to_owned())?
                .clone()
        };
        let mut request = server.borrow().debug_request();
        request.get().set_what(&what);
        let response = request.send().promise.await.map_err(map_to_string)?;
        response
            .get()
            .map_err(map_to_string)?
            .get_output()
            .map(|o| o.to_owned())
            .map_err(map_to_string)
    }

    // Start Client API connection
    pub async fn connect(&mut self, connect_addr: SocketAddr) -> Result<(), String> {
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
