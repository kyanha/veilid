use crate::*;
use connection_table::*;
use dht::*;
use futures_util::future::{select, Either};
use futures_util::stream::{FuturesUnordered, StreamExt};
use intf::*;
use lease_manager::*;
use receipt_manager::*;
use routing_table::*;
use rpc_processor::RPCProcessor;
use xx::*;

////////////////////////////////////////////////////////////////////////////////////////

const CONNECTION_PROCESSOR_CHANNEL_SIZE: usize = 128usize;

pub const MAX_MESSAGE_SIZE: usize = MAX_ENVELOPE_SIZE;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum NetworkClass {
    Server = 0,               // S = Device with public IP and no UDP firewall
    Mapped = 1,               // M = Device with portmap behind any NAT
    FullNAT = 2,              // F = Device without portmap behind full-cone NAT
    AddressRestrictedNAT = 3, // R1 = Device without portmap behind address-only restricted NAT
    PortRestrictedNAT = 4,    // R2 = Device without portmap behind address-and-port restricted NAT
    OutboundOnly = 5,         // O = Outbound only
    WebApp = 6,               // W = PWA in normal web browser
    TorWebApp = 7,            // T = PWA in Tor browser
    Invalid = 8,              // I = Invalid network class, unreachable or can not send packets
}

impl NetworkClass {
    pub fn inbound_capable(&self) -> bool {
        matches!(
            self,
            Self::Server
                | Self::Mapped
                | Self::FullNAT
                | Self::AddressRestrictedNAT
                | Self::PortRestrictedNAT
        )
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ProtocolConfig {
    pub udp_enabled: bool,
    pub tcp_connect: bool,
    pub tcp_listen: bool,
    pub ws_connect: bool,
    pub ws_listen: bool,
    pub wss_connect: bool,
    pub wss_listen: bool,
}

impl ProtocolConfig {
    pub fn is_protocol_type_connect_enabled(&self, protocol_type: ProtocolType) -> bool {
        match protocol_type {
            ProtocolType::UDP => self.udp_enabled,
            ProtocolType::TCP => self.tcp_connect,
            ProtocolType::WS => self.ws_connect,
            ProtocolType::WSS => self.wss_connect,
        }
    }
    pub fn is_protocol_type_listen_enabled(&self, protocol_type: ProtocolType) -> bool {
        match protocol_type {
            ProtocolType::UDP => self.udp_enabled,
            ProtocolType::TCP => self.tcp_listen,
            ProtocolType::WS => self.ws_listen,
            ProtocolType::WSS => self.wss_listen,
        }
    }
}

// Things we get when we start up and go away when we shut down
// Routing table is not in here because we want it to survive a network shutdown/startup restart
#[derive(Clone)]
struct NetworkComponents {
    net: Network,
    connection_table: ConnectionTable,
    rpc_processor: RPCProcessor,
    lease_manager: LeaseManager,
    receipt_manager: ReceiptManager,
}

// The mutable state of the network manager
pub struct NetworkManagerInner {
    routing_table: Option<RoutingTable>,
    components: Option<NetworkComponents>,
    network_class: Option<NetworkClass>,
    connection_processor_jh: Option<JoinHandle<()>>,
    connection_add_channel_tx: Option<utils::channel::Sender<SystemPinBoxFuture<()>>>,
}

#[derive(Clone)]
pub struct NetworkManager {
    config: VeilidConfig,
    table_store: TableStore,
    crypto: Crypto,
    inner: Arc<Mutex<NetworkManagerInner>>,
}

impl NetworkManager {
    fn new_inner() -> NetworkManagerInner {
        NetworkManagerInner {
            routing_table: None,
            components: None,
            network_class: None,
            connection_processor_jh: None,
            connection_add_channel_tx: None,
        }
    }

    pub fn new(config: VeilidConfig, table_store: TableStore, crypto: Crypto) -> Self {
        Self {
            config,
            table_store,
            crypto,
            inner: Arc::new(Mutex::new(Self::new_inner())),
        }
    }
    pub fn config(&self) -> VeilidConfig {
        self.config.clone()
    }
    pub fn table_store(&self) -> TableStore {
        self.table_store.clone()
    }
    pub fn crypto(&self) -> Crypto {
        self.crypto.clone()
    }
    pub fn routing_table(&self) -> RoutingTable {
        self.inner.lock().routing_table.as_ref().unwrap().clone()
    }
    pub fn net(&self) -> Network {
        self.inner.lock().components.as_ref().unwrap().net.clone()
    }
    pub fn rpc_processor(&self) -> RPCProcessor {
        self.inner
            .lock()
            .components
            .as_ref()
            .unwrap()
            .rpc_processor
            .clone()
    }
    pub fn lease_manager(&self) -> LeaseManager {
        self.inner
            .lock()
            .components
            .as_ref()
            .unwrap()
            .lease_manager
            .clone()
    }
    pub fn receipt_manager(&self) -> ReceiptManager {
        self.inner
            .lock()
            .components
            .as_ref()
            .unwrap()
            .receipt_manager
            .clone()
    }
    pub fn connection_table(&self) -> ConnectionTable {
        self.inner
            .lock()
            .components
            .as_ref()
            .unwrap()
            .connection_table
            .clone()
    }

    pub async fn init(&self) -> Result<(), String> {
        let routing_table = RoutingTable::new(self.clone());
        routing_table.init().await?;
        self.inner.lock().routing_table = Some(routing_table.clone());
        Ok(())
    }
    pub async fn terminate(&self) {
        let mut inner = self.inner.lock();
        if let Some(routing_table) = &inner.routing_table {
            routing_table.terminate().await;
            inner.routing_table = None;
        }
    }

    pub async fn internal_startup(&self) -> Result<(), String> {
        trace!("NetworkManager::internal_startup begin");
        if self.inner.lock().components.is_some() {
            debug!("NetworkManager::internal_startup already started");
            return Ok(());
        }

        // Create network components
        let net = Network::new(self.clone());
        let connection_table = ConnectionTable::new();
        let rpc_processor = RPCProcessor::new(self.clone());
        let lease_manager = LeaseManager::new(self.clone());
        let receipt_manager = ReceiptManager::new(self.clone());
        self.inner.lock().components = Some(NetworkComponents {
            net: net.clone(),
            connection_table: connection_table.clone(),
            rpc_processor: rpc_processor.clone(),
            lease_manager: lease_manager.clone(),
            receipt_manager: receipt_manager.clone(),
        });

        // Start network components
        rpc_processor.startup().await?;
        lease_manager.startup().await?;
        receipt_manager.startup().await?;
        net.startup().await?;

        // Run connection processing task
        let cac = utils::channel::channel(CONNECTION_PROCESSOR_CHANNEL_SIZE); // xxx move to config
        self.inner.lock().connection_add_channel_tx = Some(cac.0);
        let rx = cac.1.clone();
        let this = self.clone();
        self.inner.lock().connection_processor_jh = Some(spawn(this.connection_processor(rx)));

        trace!("NetworkManager::internal_startup end");

        Ok(())
    }

    pub async fn startup(&self) -> Result<(), String> {
        if let Err(e) = self.internal_startup().await {
            self.shutdown().await;
            return Err(e);
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
        trace!("NetworkManager::shutdown begin");
        let components = {
            let mut inner = self.inner.lock();
            // Drop/cancel the connection processing task first
            inner.connection_processor_jh = None;
            inner.connection_add_channel_tx = None;
            inner.components.clone()
        };

        // Shutdown network components if they started up
        if let Some(components) = components {
            components.net.shutdown().await;
            components.receipt_manager.shutdown().await;
            components.lease_manager.shutdown().await;
            components.rpc_processor.shutdown().await;
        }

        // reset the state
        let mut inner = self.inner.lock();
        inner.components = None;
        inner.network_class = None;

        trace!("NetworkManager::shutdown end");
    }

    pub async fn tick(&self) -> Result<(), String> {
        let (routing_table, net, lease_manager, receipt_manager) = {
            let inner = self.inner.lock();
            let components = inner.components.as_ref().unwrap();
            (
                inner.routing_table.as_ref().unwrap().clone(),
                components.net.clone(),
                components.lease_manager.clone(),
                components.receipt_manager.clone(),
            )
        };

        // If the network needs to be reset, do it
        // if things can't restart, then we fail out of the attachment manager
        if net.needs_restart() {
            net.shutdown().await;
            net.startup().await?;
        }

        // Run the routing table tick
        routing_table.tick().await?;

        // Run the low level network tick
        net.tick().await?;

        // Run the lease manager tick
        lease_manager.tick().await?;

        // Run the receipt manager tick
        receipt_manager.tick().await?;

        Ok(())
    }

    // Called by low-level protocol handlers when any connection-oriented protocol connection appears
    // either from incoming or outgoing connections. Registers connection in the connection table for later access
    // and spawns a message processing loop for the connection
    pub async fn on_new_connection(
        &self,
        descriptor: ConnectionDescriptor,
        conn: NetworkConnection,
    ) -> Result<(), String> {
        let tx = self
            .inner
            .lock()
            .connection_add_channel_tx
            .as_ref()
            .ok_or_else(fn_string!("connection channel isn't open yet"))?
            .clone();
        let this = self.clone();
        let receiver_loop_future = Self::process_connection(this, descriptor, conn);
        tx.try_send(receiver_loop_future)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed to start receiver loop"))
    }

    // Connection receiver loop
    fn process_connection(
        this: NetworkManager,
        descriptor: ConnectionDescriptor,
        conn: NetworkConnection,
    ) -> SystemPinBoxFuture<()> {
        Box::pin(async move {
            // Add new connections to the table
            let entry = match this
                .connection_table()
                .add_connection(descriptor.clone(), conn.clone())
            {
                Ok(e) => e,
                Err(err) => {
                    error!(target: "net", "{}", err);
                    return;
                }
            };

            //
            let exit_value: Result<Vec<u8>, ()> = Err(());

            loop {
                let res = match select(
                    entry.stopper.clone().instance_clone(exit_value.clone()),
                    conn.clone().recv(),
                )
                .await
                {
                    Either::Left((_x, _b)) => break,
                    Either::Right((y, _a)) => y,
                };
                let message = match res {
                    Ok(v) => v,
                    Err(_) => break,
                };
                match this.on_recv_envelope(message.as_slice(), &descriptor).await {
                    Ok(_) => (),
                    Err(e) => {
                        error!("{}", e);
                        break;
                    }
                };
            }

            if let Err(err) = this.connection_table().remove_connection(&descriptor) {
                error!("{}", err);
            }
        })
    }

    // Process connection oriented sockets in the background
    // This never terminates and must have its task cancelled once started
    async fn connection_processor(self, rx: utils::channel::Receiver<SystemPinBoxFuture<()>>) {
        let mut connection_futures: FuturesUnordered<SystemPinBoxFuture<()>> =
            FuturesUnordered::new();
        loop {
            // Either process an existing connection, or receive a new one to add to our list
            match select(connection_futures.next(), Box::pin(rx.recv())).await {
                Either::Left((x, _)) => {
                    // Processed some connection to completion, or there are none left
                    match x {
                        Some(()) => {
                            // Processed some connection to completion
                        }
                        None => {
                            // No connections to process, wait for one
                            match rx.recv().await {
                                Ok(v) => {
                                    connection_futures.push(v);
                                }
                                Err(e) => {
                                    error!("connection processor error: {:?}", e);
                                    // xxx: do something here??
                                }
                            };
                        }
                    }
                }
                Either::Right((x, _)) => {
                    // Got a new connection future
                    match x {
                        Ok(v) => {
                            connection_futures.push(v);
                        }
                        Err(e) => {
                            error!("connection processor error: {:?}", e);
                            // xxx: do something here??
                        }
                    };
                }
            }
        }
    }

    // Return what network class we are in
    pub fn get_network_class(&self) -> Option<NetworkClass> {
        if let Some(components) = &self.inner.lock().components {
            components.net.get_network_class()
        } else {
            None
        }
    }

    // Return what protocols we have enabled
    pub fn get_protocol_config(&self) -> Option<ProtocolConfig> {
        if let Some(components) = &self.inner.lock().components {
            components.net.get_protocol_config()
        } else {
            None
        }
    }

    // Generates an out-of-band receipt
    pub fn generate_receipt<D: AsRef<[u8]>>(
        &self,
        expiration_us: u64,
        expected_returns: u32,
        extra_data: D,
        callback: impl ReceiptCallback,
    ) -> Result<Vec<u8>, String> {
        let receipt_manager = self.receipt_manager();
        let routing_table = self.routing_table();

        // Generate receipt and serialized form to return
        let nonce = Crypto::get_random_nonce();
        let receipt = Receipt::try_new(0, nonce, routing_table.node_id(), extra_data)?;
        let out = receipt
            .to_signed_data(&routing_table.node_id_secret())
            .map_err(|_| "failed to generate signed receipt".to_owned())?;

        // Record the receipt for later
        let exp_ts = intf::get_timestamp() + expiration_us;
        receipt_manager.record_receipt(receipt, exp_ts, expected_returns, callback);

        Ok(out)
    }

    pub fn generate_single_shot_receipt<D: AsRef<[u8]>>(
        &self,
        expiration_us: u64,
        extra_data: D,
    ) -> Result<(Vec<u8>, EventualValueCloneFuture<ReceiptEvent>), String> {
        let receipt_manager = self.receipt_manager();
        let routing_table = self.routing_table();

        // Generate receipt and serialized form to return
        let nonce = Crypto::get_random_nonce();
        let receipt = Receipt::try_new(0, nonce, routing_table.node_id(), extra_data)?;
        let out = receipt
            .to_signed_data(&routing_table.node_id_secret())
            .map_err(|_| "failed to generate signed receipt".to_owned())?;

        // Record the receipt for later
        let exp_ts = intf::get_timestamp() + expiration_us;
        let eventual = SingleShotEventual::new(ReceiptEvent::Cancelled);
        let instance = eventual.instance();
        receipt_manager.record_single_shot_receipt(receipt, exp_ts, eventual);

        Ok((out, instance))
    }

    // Process a received out-of-band receipt
    pub async fn process_receipt<R: AsRef<[u8]>>(&self, receipt_data: R) -> Result<(), String> {
        let receipt_manager = self.receipt_manager();
        let receipt = Receipt::from_signed_data(receipt_data.as_ref())
            .map_err(|_| "failed to parse signed receipt".to_owned())?;
        receipt_manager.handle_receipt(receipt).await
    }

    // Builds an envelope for sending over the network
    fn build_envelope<B: AsRef<[u8]>>(
        &self,
        dest_node_id: key::DHTKey,
        version: u8,
        body: B,
    ) -> Result<Vec<u8>, String> {
        // DH to get encryption key
        let routing_table = self.routing_table();
        let node_id = routing_table.node_id();
        let node_id_secret = routing_table.node_id_secret();

        // Get timestamp, nonce
        let ts = intf::get_timestamp();
        let nonce = Crypto::get_random_nonce();

        // Encode envelope
        let envelope = Envelope::new(version, ts, nonce, node_id, dest_node_id);
        envelope
            .to_encrypted_data(self.crypto.clone(), body.as_ref(), &node_id_secret)
            .map_err(|_| "envelope failed to encode".to_owned())
    }

    // Called by the RPC handler when we want to issue an RPC request or response
    pub async fn send_envelope<B: AsRef<[u8]>>(
        &self,
        node_ref: NodeRef,
        body: B,
    ) -> Result<(), String> {
        // Get node's min/max version and see if we can send to it
        // and if so, get the max version we can use
        let version = if let Some((node_min, node_max)) = node_ref.operate(|e| e.min_max_version())
        {
            #[allow(clippy::absurd_extreme_comparisons)]
            if node_min > MAX_VERSION || node_max < MIN_VERSION {
                return Err(format!(
                    "can't talk to this node {} because version is unsupported: ({},{})",
                    node_ref.node_id(),
                    node_min,
                    node_max
                ));
            }
            cmp::min(node_max, MAX_VERSION)
        } else {
            MAX_VERSION
        };

        // Build the envelope to send
        let out = self.build_envelope(node_ref.node_id(), version, body)?;

        // Send via relay if we have to
        self.net().send_data(node_ref, out).await
    }

    // Called by the RPC handler when we want to issue an direct receipt
    pub async fn send_direct_receipt<B: AsRef<[u8]>>(
        &self,
        dial_info: &DialInfo,
        rcpt_data: B,
        alternate_port: bool,
    ) -> Result<(), String> {
        // Validate receipt before we send it, otherwise this may be arbitrary data!
        let _ = Receipt::from_signed_data(rcpt_data.as_ref())
            .map_err(|_| "failed to validate direct receipt".to_owned())?;

        // Send receipt directly
        if alternate_port {
            self.net()
                .send_data_unbound_to_dial_info(dial_info, rcpt_data.as_ref().to_vec())
                .await
        } else {
            self.net()
                .send_data_to_dial_info(dial_info, rcpt_data.as_ref().to_vec())
                .await
        }
    }

    // Called when a packet potentially containing an RPC envelope is received by a low-level
    // network protocol handler. Processes the envelope, authenticates and decrypts the RPC message
    // and passes it to the RPC handler
    pub async fn on_recv_envelope(
        &self,
        data: &[u8],
        descriptor: &ConnectionDescriptor,
    ) -> Result<bool, String> {
        // Is this an out-of-band receipt instead of an envelope?
        if data[0..4] == *RECEIPT_MAGIC {
            self.process_receipt(data).await?;
            return Ok(true);
        }

        // Decode envelope header
        let envelope =
            Envelope::from_data(data).map_err(|_| "envelope failed to decode".to_owned())?;

        // Get routing table and rpc processor
        let (routing_table, lease_manager, rpc) = {
            let inner = self.inner.lock();
            (
                inner.routing_table.as_ref().unwrap().clone(),
                inner.components.as_ref().unwrap().lease_manager.clone(),
                inner.components.as_ref().unwrap().rpc_processor.clone(),
            )
        };

        // Peek at header and see if we need to send this to a relay lease
        // If the recipient id is not our node id, then it needs relaying
        let sender_id = envelope.get_sender_id();
        let recipient_id = envelope.get_recipient_id();
        if recipient_id != routing_table.node_id() {
            // Ensure a lease exists for this node before we relay it
            if !lease_manager.server_has_valid_relay_lease(&recipient_id)
                && !lease_manager.server_has_valid_relay_lease(&sender_id)
            {
                return Err("received envelope not intended for this node".to_owned());
            }

            // Resolve the node to send this to
            let relay_nr = rpc.resolve_node(recipient_id).await.map_err(|e| {
                format!(
                    "failed to resolve recipient node for relay, dropping packet...: {:?}",
                    e
                )
            })?;

            // Re-send the packet to the leased node
            self.net()
                .send_data(relay_nr, data.to_vec())
                .await
                .map_err(|e| format!("failed to forward envelope: {}", e))?;
            // Inform caller that we dealt with the envelope, but did not process it locally
            return Ok(false);
        }

        // DH to get decryption key (cached)
        let node_id_secret = routing_table.node_id_secret();

        // Decrypt the envelope body
        // xxx: punish nodes that send messages that fail to decrypt eventually
        let body = envelope
            .decrypt_body(self.crypto(), data, &node_id_secret)
            .map_err(|_| "failed to decrypt envelope body".to_owned())?;

        // Get timestamp range
        let (tsbehind, tsahead) = {
            let c = self.config.get();
            (
                c.network.rpc.max_timestamp_behind,
                c.network.rpc.max_timestamp_ahead,
            )
        };

        // Validate timestamp isn't too old
        let ts = intf::get_timestamp();
        let ets = envelope.get_timestamp();
        if let Some(tsbehind) = tsbehind {
            if tsbehind > 0 && (ts > ets && ts - ets > tsbehind) {
                return Err(format!(
                    "envelope time was too far in the past: {}ms ",
                    timestamp_to_secs(ts - ets) * 1000f64
                ));
            }
        }
        if let Some(tsahead) = tsahead {
            if tsahead > 0 && (ts < ets && ets - ts > tsahead) {
                return Err(format!(
                    "envelope time was too far in the future: {}ms",
                    timestamp_to_secs(ets - ts) * 1000f64
                ));
            }
        }

        // Cache the envelope information in the routing table
        let source_noderef = routing_table
            .register_node_with_existing_connection(
                envelope.get_sender_id(),
                descriptor.clone(),
                ts,
            )
            .map_err(|e| format!("node id registration failed: {}", e))?;
        source_noderef.operate(|e| e.set_min_max_version(envelope.get_min_max_version()));

        // xxx: deal with spoofing and flooding here?

        // Pass message to RPC system
        rpc.enqueue_message(envelope, body, source_noderef)
            .await
            .map_err(|e| format!("enqueing rpc message failed: {}", e))?;

        // Inform caller that we dealt with the envelope locally
        Ok(true)
    }
}
