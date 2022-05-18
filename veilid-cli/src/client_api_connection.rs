use crate::command_processor::*;
use crate::veilid_client_capnp::*;
use async_executors::{AsyncStd, LocalSpawnHandleExt};
use capnp::capability::Promise;
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, Disconnector, RpcSystem};
use futures::io::AsyncReadExt;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;
use veilid_core::xx::*;

macro_rules! capnp_failed {
    ($ex:expr) => {{
        let msg = format!("Capnp Error: {}", $ex);
        error!("{}", msg);
        Promise::err(capnp::Error::failed(msg))
    }};
}

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
                return capnp_failed!(format!("(missing update kind in schema: {:?})", e));
            }
        };
        match which {
            veilid_update::Attachment(Ok(attachment)) => {
                let state = pry!(attachment.get_state());

                trace!("Attachment: {}", state as u16);
                self.comproc.update_attachment(state);
            }
            veilid_update::Attachment(Err(e)) => {
                return capnp_failed!(format!("Update Attachment Error: {}", e));
            }
            veilid_update::Network(Ok(network)) => {
                let started = network.get_started();
                let bps_down = network.get_bps_down();
                let bps_up = network.get_bps_up();

                trace!(
                    "Network: started: {}  bps_down: {}  bps_up: {}",
                    started,
                    bps_down,
                    bps_up
                );
                self.comproc
                    .update_network_status(started, bps_down, bps_up);
            }
            veilid_update::Network(Err(e)) => {
                return capnp_failed!(format!("Update Network Error: {}", e));
            }
            veilid_update::Shutdown(()) => {
                return capnp_failed!("Should not get Shutdown here".to_owned());
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
        veilid_state: veilid_state::Reader<'a>,
    ) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();

        // Process attachment state
        let attachment = veilid_state
            .reborrow()
            .get_attachment()
            .map_err(map_to_string)?;
        let attachment_state = attachment.get_state().map_err(map_to_string)?;

        let network = veilid_state
            .reborrow()
            .get_network()
            .map_err(map_to_string)?;
        let started = network.get_started();
        let bps_down = network.get_bps_down();
        let bps_up = network.get_bps_up();

        inner.comproc.update_attachment(attachment_state);
        inner
            .comproc
            .update_network_status(started, bps_down, bps_up);

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

        // Process the rpc system until we decide we're done
        if let Ok(rpc_jh) = AsyncStd.spawn_handle_local(rpc_system) {
            // Send the request and get the state object and the registration object
            if let Ok(response) = request.send().promise.await {
                if let Ok(response) = response.get() {
                    if let Ok(_registration) = response.get_registration() {
                        if let Ok(state) = response.get_state() {
                            // Set up our state for the first time
                            if self.process_veilid_state(state).await.is_ok() {
                                // Don't drop the registration, doing so will remove the client
                                // object mapping from the server which we need for the update backchannel

                                // Wait until rpc system completion or disconnect was requested
                                if let Err(e) = rpc_jh.await {
                                    error!("Client RPC system error: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            error!("Failed to spawn client RPC system");
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
