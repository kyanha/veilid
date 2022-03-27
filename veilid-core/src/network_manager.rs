use crate::*;
use connection_manager::*;
use dht::*;
use hashlink::LruCache;
use intf::*;
use lease_manager::*;
use receipt_manager::*;
use routing_table::*;
use rpc_processor::RPCProcessor;
use xx::*;

////////////////////////////////////////////////////////////////////////////////////////

pub const MAX_MESSAGE_SIZE: usize = MAX_ENVELOPE_SIZE;
pub const IPADDR_TABLE_SIZE: usize = 1024;
pub const IPADDR_MAX_INACTIVE_DURATION_US: u64 = 300_000_000u64; // 5 minutes

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
    connection_manager: ConnectionManager,
    rpc_processor: RPCProcessor,
    lease_manager: LeaseManager,
    receipt_manager: ReceiptManager,
}

// Statistics per address
#[derive(Clone, Default)]
pub struct PerAddressStats {
    last_seen_ts: u64,
    transfer_stats_accounting: TransferStatsAccounting,
    transfer_stats: TransferStatsDownUp,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PerAddressStatsKey(IpAddr);

impl Default for PerAddressStatsKey {
    fn default() -> Self {
        Self(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
    }
}

// Statistics about the low-level network
#[derive(Clone)]
pub struct NetworkManagerStats {
    self_stats: PerAddressStats,
    per_address_stats: LruCache<PerAddressStatsKey, PerAddressStats>,
}

impl Default for NetworkManagerStats {
    fn default() -> Self {
        Self {
            self_stats: PerAddressStats::default(),
            per_address_stats: LruCache::new(IPADDR_TABLE_SIZE),
        }
    }
}
// The mutable state of the network manager
struct NetworkManagerInner {
    routing_table: Option<RoutingTable>,
    components: Option<NetworkComponents>,
    network_class: Option<NetworkClass>,
    stats: NetworkManagerStats,
}

struct NetworkManagerUnlockedInner {
    // Background processes
    rolling_transfers_task: TickTask,
}

#[derive(Clone)]
pub struct NetworkManager {
    config: VeilidConfig,
    table_store: TableStore,
    crypto: Crypto,
    inner: Arc<Mutex<NetworkManagerInner>>,
    unlocked_inner: Arc<NetworkManagerUnlockedInner>,
}

impl NetworkManager {
    fn new_inner() -> NetworkManagerInner {
        NetworkManagerInner {
            routing_table: None,
            components: None,
            network_class: None,
            stats: NetworkManagerStats::default(),
        }
    }
    fn new_unlocked_inner(_config: VeilidConfig) -> NetworkManagerUnlockedInner {
        //let c = config.get();
        NetworkManagerUnlockedInner {
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
        }
    }

    pub fn new(config: VeilidConfig, table_store: TableStore, crypto: Crypto) -> Self {
        let this = Self {
            config: config.clone(),
            table_store,
            crypto,
            inner: Arc::new(Mutex::new(Self::new_inner())),
            unlocked_inner: Arc::new(Self::new_unlocked_inner(config)),
        };
        // Set rolling transfers tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .rolling_transfers_task
                .set_routine(move |l, t| {
                    Box::pin(this2.clone().rolling_transfers_task_routine(l, t))
                });
        }
        this
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
    pub fn connection_manager(&self) -> ConnectionManager {
        self.inner
            .lock()
            .components
            .as_ref()
            .unwrap()
            .connection_manager
            .clone()
    }

    pub async fn init(&self) -> Result<(), String> {
        let routing_table = RoutingTable::new(self.clone());
        routing_table.init().await?;
        self.inner.lock().routing_table = Some(routing_table.clone());
        Ok(())
    }
    pub async fn terminate(&self) {
        let routing_table = {
            let mut inner = self.inner.lock();
            inner.routing_table.take()
        };
        if let Some(routing_table) = routing_table {
            routing_table.terminate().await;
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
        let connection_manager = ConnectionManager::new(self.clone());
        let rpc_processor = RPCProcessor::new(self.clone());
        let lease_manager = LeaseManager::new(self.clone());
        let receipt_manager = ReceiptManager::new(self.clone());
        self.inner.lock().components = Some(NetworkComponents {
            net: net.clone(),
            connection_manager: connection_manager.clone(),
            rpc_processor: rpc_processor.clone(),
            lease_manager: lease_manager.clone(),
            receipt_manager: receipt_manager.clone(),
        });

        // Start network components
        rpc_processor.startup().await?;
        lease_manager.startup().await?;
        receipt_manager.startup().await?;
        net.startup().await?;
        connection_manager.startup().await;

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

        // Shutdown network components if they started up
        let components = self.inner.lock().components.clone();
        if let Some(components) = components {
            components.connection_manager.shutdown().await;
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
        log_net!("sending envelope to {:?}", node_ref);
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
                ))
                .map_err(logthru_rpc!(warn));
            }
            cmp::min(node_max, MAX_VERSION)
        } else {
            MAX_VERSION
        };

        // Build the envelope to send
        let out = self
            .build_envelope(node_ref.node_id(), version, body)
            .map_err(logthru_rpc!(error))?;

        // Send via relay if we have to
        self.net().send_data(node_ref, out).await
    }

    // Called by the RPC handler when we want to issue an direct receipt
    pub async fn send_direct_receipt<B: AsRef<[u8]>>(
        &self,
        dial_info: DialInfo,
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
        descriptor: ConnectionDescriptor,
    ) -> Result<bool, String> {
        log_net!(
            "envelope of {} bytes received from {:?}",
            data.len(),
            descriptor
        );

        // Network accounting
        self.stats_packet_rcvd(descriptor.remote.to_socket_addr().ip(), data.len() as u64);

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
            let relay_nr = if let Some(lease_nr) =
                lease_manager.server_has_valid_relay_lease(&recipient_id)
            {
                // Inbound lease
                lease_nr
            } else if let Some(lease_nr) = lease_manager.server_has_valid_relay_lease(&sender_id) {
                // Resolve the node to send this to
                rpc.resolve_node(recipient_id, Some(lease_nr.clone())).await.map_err(|e| {
                    format!(
                        "failed to resolve recipient node for relay, dropping outbound relayed packet...: {:?}",
                        e
                    )
                })?
            } else {
                return Err("received envelope not intended for this node".to_owned());
            };

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
                c.network.rpc.max_timestamp_behind_ms.map(ms_to_us),
                c.network.rpc.max_timestamp_ahead_ms.map(ms_to_us),
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
            .register_node_with_existing_connection(envelope.get_sender_id(), descriptor, ts)
            .map_err(|e| format!("node id registration failed: {}", e))?;
        source_noderef.operate(|e| e.set_min_max_version(envelope.get_min_max_version()));

        // xxx: deal with spoofing and flooding here?

        // Pass message to RPC system
        rpc.enqueue_message(envelope, body, source_noderef)
            .map_err(|e| format!("enqueing rpc message failed: {}", e))?;

        // Inform caller that we dealt with the envelope locally
        Ok(true)
    }

    // Compute transfer statistics for the low level network
    async fn rolling_transfers_task_routine(self, last_ts: u64, cur_ts: u64) -> Result<(), String> {
        log_net!("--- network manager rolling_transfers task");
        let inner = &mut *self.inner.lock();

        // Roll the low level network transfer stats for our address
        inner
            .stats
            .self_stats
            .transfer_stats_accounting
            .roll_transfers(last_ts, cur_ts, &mut inner.stats.self_stats.transfer_stats);

        // Roll all per-address transfers
        let mut dead_addrs: HashSet<PerAddressStatsKey> = HashSet::new();
        for (addr, stats) in &mut inner.stats.per_address_stats {
            stats.transfer_stats_accounting.roll_transfers(
                last_ts,
                cur_ts,
                &mut stats.transfer_stats,
            );

            // While we're here, lets see if this address has timed out
            if cur_ts - stats.last_seen_ts >= IPADDR_MAX_INACTIVE_DURATION_US {
                // it's dead, put it in the dead list
                dead_addrs.insert(*addr);
            }
        }

        // Remove the dead addresses from our tables
        for da in &dead_addrs {
            inner.stats.per_address_stats.remove(da);
        }
        Ok(())
    }

    // Callbacks from low level network for statistics gathering
    pub fn stats_packet_sent(&self, addr: IpAddr, bytes: u64) {
        let inner = &mut *self.inner.lock();
        inner
            .stats
            .self_stats
            .transfer_stats_accounting
            .add_up(bytes);
        inner
            .stats
            .per_address_stats
            .entry(PerAddressStatsKey(addr))
            .or_insert(PerAddressStats::default())
            .transfer_stats_accounting
            .add_up(bytes);
    }

    pub fn stats_packet_rcvd(&self, addr: IpAddr, bytes: u64) {
        let inner = &mut *self.inner.lock();
        inner
            .stats
            .self_stats
            .transfer_stats_accounting
            .add_down(bytes);
        inner
            .stats
            .per_address_stats
            .entry(PerAddressStatsKey(addr))
            .or_insert(PerAddressStats::default())
            .transfer_stats_accounting
            .add_down(bytes);
    }
}
