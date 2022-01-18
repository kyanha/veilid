use crate::veilid_client_capnp::*;
use async_std::net::TcpListener;
use async_std::prelude::FutureExt;
use capnp::capability::Promise;
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, RpcSystem};
use failure::*;
use futures::FutureExt as FuturesFutureExt;
use futures::{AsyncReadExt, StreamExt};
use log::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use veilid_core::xx::Eventual;

#[derive(Fail, Debug)]
#[fail(display = "Client API error: {}", _0)]
pub struct ClientAPIError(String);

fn convert_attachment_state(state: &veilid_core::AttachmentState) -> AttachmentState {
    match state {
        veilid_core::AttachmentState::Detached => AttachmentState::Detached,
        veilid_core::AttachmentState::Attaching => AttachmentState::Attaching,
        veilid_core::AttachmentState::AttachedWeak => AttachmentState::AttachedWeak,
        veilid_core::AttachmentState::AttachedGood => AttachmentState::AttachedGood,
        veilid_core::AttachmentState::AttachedStrong => AttachmentState::AttachedStrong,
        veilid_core::AttachmentState::FullyAttached => AttachmentState::FullyAttached,
        veilid_core::AttachmentState::OverAttached => AttachmentState::OverAttached,
        veilid_core::AttachmentState::Detaching => AttachmentState::Detaching,
    }
}

fn convert_update(
    update: &veilid_core::VeilidUpdate,
    rpc_update: crate::veilid_client_capnp::veilid_update::Builder,
) {
    match update {
        veilid_core::VeilidUpdate::Attachment(state) => {
            let mut att = rpc_update.init_attachment();
            att.set_state(convert_attachment_state(state));
        }
    }
}

fn convert_state(
    state: &veilid_core::VeilidState,
    rpc_state: crate::veilid_client_capnp::veilid_state::Builder,
) {
    rpc_state
        .init_attachment()
        .set_state(convert_attachment_state(&state.attachment));
}

// --- interface Registration ---------------------------------

struct RegistrationHandle {
    client: veilid_client::Client,
    requests_in_flight: i32,
}

struct RegistrationMap {
    registrations: HashMap<u64, RegistrationHandle>,
}

impl RegistrationMap {
    fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }
}

struct RegistrationImpl {
    id: u64,
    registration_map: Rc<RefCell<RegistrationMap>>,
}

impl RegistrationImpl {
    fn new(id: u64, registrations: Rc<RefCell<RegistrationMap>>) -> Self {
        Self {
            id,
            registration_map: registrations,
        }
    }
}

impl Drop for RegistrationImpl {
    fn drop(&mut self) {
        debug!("Registration dropped");
        self.registration_map
            .borrow_mut()
            .registrations
            .remove(&self.id);
    }
}

impl registration::Server for RegistrationImpl {}

// --- interface VeilidServer ---------------------------------

struct VeilidServerImpl {
    veilid_api: veilid_core::VeilidAPI,
    next_id: u64,
    pub registration_map: Rc<RefCell<RegistrationMap>>,
}

impl VeilidServerImpl {
    pub fn new(veilid_api: veilid_core::VeilidAPI) -> Self {
        Self {
            next_id: 0,
            registration_map: Rc::new(RefCell::new(RegistrationMap::new())),
            veilid_api,
        }
    }
}

impl veilid_server::Server for VeilidServerImpl {
    fn register(
        &mut self,
        params: veilid_server::RegisterParams,
        mut results: veilid_server::RegisterResults,
    ) -> Promise<(), ::capnp::Error> {
        trace!("VeilidServerImpl::register");

        self.registration_map.borrow_mut().registrations.insert(
            self.next_id,
            RegistrationHandle {
                client: pry!(pry!(params.get()).get_veilid_client()),
                requests_in_flight: 0,
            },
        );

        let veilid_api = self.veilid_api.clone();
        let registration = capnp_rpc::new_client(RegistrationImpl::new(
            self.next_id,
            self.registration_map.clone(),
        ));
        self.next_id += 1;

        Promise::from_future(async move {
            let state = veilid_api
                .get_state()
                .await
                .map_err(|e| ::capnp::Error::failed(format!("{:?}", e)))?;

            let mut res = results.get();
            res.set_registration(registration);
            let rpc_state = res.init_state();
            convert_state(&state, rpc_state);

            Ok(())
        })
    }

    fn debug(
        &mut self,
        params: veilid_server::DebugParams,
        mut results: veilid_server::DebugResults,
    ) -> Promise<(), ::capnp::Error> {
        trace!("VeilidServerImpl::debug");
        let veilid_api = self.veilid_api.clone();
        let what = pry!(pry!(params.get()).get_what()).to_owned();

        Promise::from_future(async move {
            let output = veilid_api
                .debug(what)
                .await
                .map_err(|e| ::capnp::Error::failed(format!("{:?}", e)))?;
            results.get().set_output(output.as_str());
            Ok(())
        })
    }

    fn attach(
        &mut self,
        _params: veilid_server::AttachParams,
        mut _results: veilid_server::AttachResults,
    ) -> Promise<(), ::capnp::Error> {
        trace!("VeilidServerImpl::attach");
        let veilid_api = self.veilid_api.clone();
        Promise::from_future(async move {
            veilid_api
                .attach()
                .await
                .map_err(|e| ::capnp::Error::failed(format!("{:?}", e)))
        })
    }

    fn detach(
        &mut self,
        _params: veilid_server::DetachParams,
        mut _results: veilid_server::DetachResults,
    ) -> Promise<(), ::capnp::Error> {
        trace!("VeilidServerImpl::detach");
        let veilid_api = self.veilid_api.clone();
        Promise::from_future(async move {
            veilid_api
                .detach()
                .await
                .map_err(|e| ::capnp::Error::failed(format!("{:?}", e)))
        })
    }

    fn shutdown(
        &mut self,
        _params: veilid_server::ShutdownParams,
        mut _results: veilid_server::ShutdownResults,
    ) -> Promise<(), ::capnp::Error> {
        trace!("VeilidServerImpl::shutdown");

        cfg_if::cfg_if! {
            if #[cfg(windows)] {
                assert!(false, "write me!");
            }
            else {
                crate::server::shutdown();
            }
        }

        Promise::ok(())
    }

    fn get_state(
        &mut self,
        _params: veilid_server::GetStateParams,
        mut results: veilid_server::GetStateResults,
    ) -> Promise<(), ::capnp::Error> {
        trace!("VeilidServerImpl::get_state");
        let veilid_api = self.veilid_api.clone();
        Promise::from_future(async move {
            let state = veilid_api
                .get_state()
                .await
                .map_err(|e| ::capnp::Error::failed(format!("{:?}", e)))?;

            let rpc_state = results.get().init_state();
            convert_state(&state, rpc_state);
            Ok(())
        })
    }
}

// --- Client API Server-Side ---------------------------------

type ClientApiAllFuturesJoinHandle =
    async_std::task::JoinHandle<Result<Vec<()>, Box<(dyn std::error::Error + 'static)>>>;

struct ClientApiInner {
    veilid_api: veilid_core::VeilidAPI,
    registration_map: Rc<RefCell<RegistrationMap>>,
    stop: Eventual,
    join_handle: Option<ClientApiAllFuturesJoinHandle>,
}

pub struct ClientApi {
    inner: RefCell<ClientApiInner>,
}

impl ClientApi {
    pub fn new(veilid_api: veilid_core::VeilidAPI) -> Rc<Self> {
        Rc::new(Self {
            inner: RefCell::new(ClientApiInner {
                veilid_api,
                registration_map: Rc::new(RefCell::new(RegistrationMap::new())),
                stop: Eventual::new(),
                join_handle: None,
            }),
        })
    }

    pub async fn stop(self: Rc<Self>) {
        trace!("ClientApi::stop requested");
        let jh = {
            let mut inner = self.inner.borrow_mut();
            if inner.join_handle.is_none() {
                trace!("ClientApi stop ignored");
                return;
            }
            inner.stop.resolve();
            inner.join_handle.take().unwrap()
        };
        trace!("ClientApi::stop: waiting for stop");
        if let Err(err) = jh.await {
            error!("{}", err);
        }
        trace!("ClientApi::stop: stopped");
    }

    async fn handle_incoming(
        self: Rc<Self>,
        bind_addr: SocketAddr,
        client: veilid_server::Client,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(bind_addr).await?;
        debug!("Client API listening on: {:?}", bind_addr);

        // Get the
        let mut incoming = listener.incoming();
        let stop = self.inner.borrow().stop.clone();
        let incoming_loop = async move {
            while let Some(stream_result) = stop.instance_none().race(incoming.next()).await {
                let stream = stream_result?;
                stream.set_nodelay(true)?;
                let (reader, writer) = stream.split();
                let network = twoparty::VatNetwork::new(
                    reader,
                    writer,
                    rpc_twoparty_capnp::Side::Server,
                    Default::default(),
                );

                let rpc_system = RpcSystem::new(Box::new(network), Some(client.clone().client));

                async_std::task::spawn_local(rpc_system.map(drop));
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        };

        incoming_loop.await
    }

    fn send_request_to_all_clients<F, T>(self: Rc<Self>, request: F)
    where
        F: Fn(u64, &mut RegistrationHandle) -> ::capnp::capability::RemotePromise<T>,
        T: capnp::traits::Pipelined + for<'a> capnp::traits::Owned<'a> + 'static + Unpin,
    {
        // Send status update to each registered client
        let registration_map = self.inner.borrow().registration_map.clone();
        let registration_map1 = registration_map.clone();
        let regs = &mut registration_map.borrow_mut().registrations;
        for (&id, mut registration) in regs.iter_mut() {
            if registration.requests_in_flight > 5 {
                println!(
                    "too many requests in flight: {}",
                    registration.requests_in_flight
                );
            }
            registration.requests_in_flight += 1;

            let request_promise = request(id, registration);

            let registration_map2 = registration_map1.clone();
            async_std::task::spawn_local(request_promise.promise.map(move |r| match r {
                Ok(_) => {
                    if let Some(ref mut s) =
                        registration_map2.borrow_mut().registrations.get_mut(&id)
                    {
                        s.requests_in_flight -= 1;
                    }
                }
                Err(e) => {
                    println!("Got error: {:?}. Dropping registation.", e);
                    registration_map2.borrow_mut().registrations.remove(&id);
                }
            }));
        }
    }

    pub fn handle_update(self: Rc<Self>, veilid_update: veilid_core::VeilidUpdate) {
        self.send_request_to_all_clients(|_id, registration| {
            let mut request = registration.client.update_request();
            let rpc_veilid_update = request.get().init_veilid_update();
            convert_update(&veilid_update, rpc_veilid_update);
            request.send()
        });
    }

    pub fn handle_client_log(self: Rc<Self>, message: String) {
        self.send_request_to_all_clients(|_id, registration| {
            let mut request = registration.client.log_message_request();
            request.get().set_message(&message);
            request.send()
        });
    }

    pub fn run(self: Rc<Self>, bind_addrs: Vec<SocketAddr>) {
        // Create client api VeilidServer
        let veilid_server_impl = VeilidServerImpl::new(self.inner.borrow().veilid_api.clone());
        self.inner.borrow_mut().registration_map = veilid_server_impl.registration_map.clone();

        // Make a client object for the server to send to each rpc client
        let client: veilid_server::Client = capnp_rpc::new_client(veilid_server_impl);

        let bind_futures = bind_addrs
            .iter()
            .map(|addr| self.clone().handle_incoming(*addr, client.clone()));
        let bind_futures_join = futures::future::try_join_all(bind_futures);
        self.inner.borrow_mut().join_handle = Some(async_std::task::spawn_local(bind_futures_join));
    }
}
