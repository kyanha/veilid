use crate::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

mod connection_handle;
mod connection_limits;
mod connection_manager;
mod connection_table;
mod network_connection;

pub mod tests;

////////////////////////////////////////////////////////////////////////////////////////

pub use network_connection::*;

////////////////////////////////////////////////////////////////////////////////////////
use connection_handle::*;
use connection_limits::*;
use connection_manager::*;
use dht::*;
use hashlink::LruCache;
use intf::*;
#[cfg(not(target_arch = "wasm32"))]
use native::*;
use receipt_manager::*;
use routing_table::*;
use rpc_processor::*;
#[cfg(target_arch = "wasm32")]
use wasm::*;
use xx::*;

////////////////////////////////////////////////////////////////////////////////////////

pub const RELAY_MANAGEMENT_INTERVAL_SECS: u32 = 1;
pub const MAX_MESSAGE_SIZE: usize = MAX_ENVELOPE_SIZE;
pub const IPADDR_TABLE_SIZE: usize = 1024;
pub const IPADDR_MAX_INACTIVE_DURATION_US: u64 = 300_000_000u64; // 5 minutes
pub const GLOBAL_ADDRESS_CHANGE_DETECTION_COUNT: usize = 3;

#[derive(Copy, Clone, Debug, Default)]
pub struct ProtocolConfig {
    pub outbound: ProtocolSet,
    pub inbound: ProtocolSet,
}

// Things we get when we start up and go away when we shut down
// Routing table is not in here because we want it to survive a network shutdown/startup restart
#[derive(Clone)]
struct NetworkComponents {
    net: Network,
    connection_manager: ConnectionManager,
    rpc_processor: RPCProcessor,
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

struct ClientWhitelistEntry {
    last_seen_ts: u64,
}

// Mechanism required to contact another node
#[derive(Clone, Debug)]
enum ContactMethod {
    Unreachable,                       // Node is not reachable by any means
    Direct(DialInfo),                  // Contact the node directly
    SignalReverse(NodeRef, NodeRef),   // Request via signal the node connect back directly
    SignalHolePunch(NodeRef, NodeRef), // Request via signal the node negotiate a hole punch
    InboundRelay(NodeRef),             // Must use an inbound relay to reach the node
    OutboundRelay(NodeRef),            // Must use outbound relay to reach the node
}

#[derive(Copy, Clone, Debug)]
pub enum SendDataKind {
    LocalDirect,
    GlobalDirect,
    GlobalIndirect,
}

// The mutable state of the network manager
struct NetworkManagerInner {
    routing_table: Option<RoutingTable>,
    components: Option<NetworkComponents>,
    update_callback: Option<UpdateCallback>,
    stats: NetworkManagerStats,
    client_whitelist: LruCache<DHTKey, ClientWhitelistEntry>,
    relay_node: Option<NodeRef>,
    public_address_check_cache: LruCache<DHTKey, SocketAddress>,
}

struct NetworkManagerUnlockedInner {
    // Background processes
    rolling_transfers_task: TickTask,
    relay_management_task: TickTask,
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
            update_callback: None,
            stats: NetworkManagerStats::default(),
            client_whitelist: LruCache::new_unbounded(),
            relay_node: None,
            public_address_check_cache: LruCache::new(8),
        }
    }
    fn new_unlocked_inner(_config: VeilidConfig) -> NetworkManagerUnlockedInner {
        //let c = config.get();
        NetworkManagerUnlockedInner {
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
            relay_management_task: TickTask::new(RELAY_MANAGEMENT_INTERVAL_SECS),
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
        // Set relay management tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .relay_management_task
                .set_routine(move |l, t| {
                    Box::pin(this2.clone().relay_management_task_routine(l, t))
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

    pub fn relay_node(&self) -> Option<NodeRef> {
        self.inner.lock().relay_node.clone()
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn init(&self, update_callback: UpdateCallback) -> Result<(), String> {
        let routing_table = RoutingTable::new(self.clone());
        routing_table.init().await?;
        self.inner.lock().routing_table = Some(routing_table.clone());
        self.inner.lock().update_callback = Some(update_callback);
        Ok(())
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn terminate(&self) {
        let routing_table = {
            let mut inner = self.inner.lock();
            inner.routing_table.take()
        };
        if let Some(routing_table) = routing_table {
            routing_table.terminate().await;
        }
        self.inner.lock().update_callback = None;
    }

    #[instrument(level = "debug", skip_all, err)]
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
        let receipt_manager = ReceiptManager::new(self.clone());
        self.inner.lock().components = Some(NetworkComponents {
            net: net.clone(),
            connection_manager: connection_manager.clone(),
            rpc_processor: rpc_processor.clone(),
            receipt_manager: receipt_manager.clone(),
        });

        // Start network components
        rpc_processor.startup().await?;
        receipt_manager.startup().await?;
        net.startup().await?;
        connection_manager.startup().await;

        trace!("NetworkManager::internal_startup end");

        Ok(())
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn startup(&self) -> Result<(), String> {
        if let Err(e) = self.internal_startup().await {
            self.shutdown().await;
            return Err(e);
        }

        self.send_network_update();

        Ok(())
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn shutdown(&self) {
        trace!("NetworkManager::shutdown begin");

        // Cancel all tasks
        if let Err(e) = self.unlocked_inner.rolling_transfers_task.cancel().await {
            warn!("rolling_transfers_task not cancelled: {}", e);
        }
        if let Err(e) = self.unlocked_inner.relay_management_task.cancel().await {
            warn!("relay_management_task not cancelled: {}", e);
        }

        // Shutdown network components if they started up
        let components = self.inner.lock().components.clone();
        if let Some(components) = components {
            components.connection_manager.shutdown().await;
            components.net.shutdown().await;
            components.receipt_manager.shutdown().await;
            components.rpc_processor.shutdown().await;
        }

        // reset the state
        {
            let mut inner = self.inner.lock();
            inner.components = None;
            inner.relay_node = None;
        }

        // send update
        self.send_network_update();

        trace!("NetworkManager::shutdown end");
    }

    pub fn update_client_whitelist(&self, client: DHTKey) {
        let mut inner = self.inner.lock();
        match inner.client_whitelist.entry(client) {
            hashlink::lru_cache::Entry::Occupied(mut entry) => {
                entry.get_mut().last_seen_ts = intf::get_timestamp()
            }
            hashlink::lru_cache::Entry::Vacant(entry) => {
                entry.insert(ClientWhitelistEntry {
                    last_seen_ts: intf::get_timestamp(),
                });
            }
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub fn check_client_whitelist(&self, client: DHTKey) -> bool {
        let mut inner = self.inner.lock();

        match inner.client_whitelist.entry(client) {
            hashlink::lru_cache::Entry::Occupied(mut entry) => {
                entry.get_mut().last_seen_ts = intf::get_timestamp();
                true
            }
            hashlink::lru_cache::Entry::Vacant(_) => false,
        }
    }

    #[instrument(level = "trace", skip(self))]
    pub fn purge_client_whitelist(&self) {
        let timeout_ms = self.config.get().network.client_whitelist_timeout_ms;
        let mut inner = self.inner.lock();
        let cutoff_timestamp = intf::get_timestamp() - ((timeout_ms as u64) * 1000u64);
        // Remove clients from the whitelist that haven't been since since our whitelist timeout
        while inner
            .client_whitelist
            .peek_lru()
            .map(|v| v.1.last_seen_ts < cutoff_timestamp)
            .unwrap_or_default()
        {
            inner.client_whitelist.remove_lru();
        }
    }

    #[instrument(level = "debug", skip_all, err)]
    async fn restart_net(&self, net: Network) -> Result<(), String> {
        net.shutdown().await;
        self.send_network_update();
        net.startup().await?;
        self.send_network_update();
        Ok(())
    }

    pub async fn tick(&self) -> Result<(), String> {
        let (routing_table, net, receipt_manager) = {
            let inner = self.inner.lock();
            let components = inner.components.as_ref().unwrap();
            (
                inner.routing_table.as_ref().unwrap().clone(),
                components.net.clone(),
                components.receipt_manager.clone(),
            )
        };

        // If the network needs to be reset, do it
        // if things can't restart, then we fail out of the attachment manager
        if net.needs_restart() {
            self.restart_net(net.clone()).await?;
        }

        // Run the rolling transfers task
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // Run the relay management task
        self.unlocked_inner.relay_management_task.tick().await?;

        // Run the routing table tick
        routing_table.tick().await?;

        // Run the low level network tick
        net.tick().await?;

        // Run the receipt manager tick
        receipt_manager.tick().await?;

        // Purge the client whitelist
        self.purge_client_whitelist();

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

    // Get our node's capabilities
    pub fn generate_node_status(&self) -> NodeStatus {
        let peer_info = self.routing_table().get_own_peer_info();

        let will_route = peer_info.signed_node_info.node_info.can_inbound_relay(); // xxx: eventually this may have more criteria added
        let will_tunnel = peer_info.signed_node_info.node_info.can_inbound_relay(); // xxx: we may want to restrict by battery life and network bandwidth at some point
        let will_signal = peer_info.signed_node_info.node_info.can_signal();
        let will_relay = peer_info.signed_node_info.node_info.can_inbound_relay();
        let will_validate_dial_info = peer_info
            .signed_node_info
            .node_info
            .can_validate_dial_info();

        NodeStatus {
            will_route,
            will_tunnel,
            will_signal,
            will_relay,
            will_validate_dial_info,
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

    // Generates a multi-shot/normal receipt
    #[instrument(level = "trace", skip(self, extra_data, callback), err)]
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

    // Generates a single-shot/normal receipt
    #[instrument(level = "trace", skip(self, extra_data), err)]
    pub fn generate_single_shot_receipt<D: AsRef<[u8]>>(
        &self,
        expiration_us: u64,
        extra_data: D,
    ) -> Result<(Vec<u8>, EventualValueFuture<ReceiptEvent>), String> {
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
        let eventual = SingleShotEventual::new(Some(ReceiptEvent::Cancelled));
        let instance = eventual.instance();
        receipt_manager.record_single_shot_receipt(receipt, exp_ts, eventual);

        Ok((out, instance))
    }

    // Process a received out-of-band receipt
    #[instrument(level = "trace", skip(self, receipt_data), err)]
    pub async fn handle_out_of_band_receipt<R: AsRef<[u8]>>(
        &self,
        receipt_data: R,
    ) -> Result<(), String> {
        let receipt_manager = self.receipt_manager();

        let receipt = Receipt::from_signed_data(receipt_data.as_ref())
            .map_err(|_| "failed to parse signed out-of-band receipt".to_owned())?;

        receipt_manager.handle_receipt(receipt, None).await
    }

    // Process a received in-band receipt
    #[instrument(level = "trace", skip(self, receipt_data), err)]
    pub async fn handle_in_band_receipt<R: AsRef<[u8]>>(
        &self,
        receipt_data: R,
        inbound_nr: NodeRef,
    ) -> Result<(), String> {
        let receipt_manager = self.receipt_manager();

        let receipt = Receipt::from_signed_data(receipt_data.as_ref())
            .map_err(|_| "failed to parse signed in-band receipt".to_owned())?;

        receipt_manager
            .handle_receipt(receipt, Some(inbound_nr))
            .await
    }

    // Process a received signal
    #[instrument(level = "trace", skip(self), err)]
    pub async fn handle_signal(&self, signal_info: SignalInfo) -> Result<(), String> {
        match signal_info {
            SignalInfo::ReverseConnect { receipt, peer_info } => {
                let routing_table = self.routing_table();
                let rpc = self.rpc_processor();

                // Add the peer info to our routing table
                let peer_nr = routing_table.register_node_with_signed_node_info(
                    peer_info.node_id.key,
                    peer_info.signed_node_info,
                )?;

                // Make a reverse connection to the peer and send the receipt to it
                rpc.rpc_call_return_receipt(Destination::Direct(peer_nr), None, receipt)
                    .await
                    .map_err(map_to_string)?;
            }
            SignalInfo::HolePunch { receipt, peer_info } => {
                let routing_table = self.routing_table();
                let rpc = self.rpc_processor();

                // Add the peer info to our routing table
                let mut peer_nr = routing_table.register_node_with_signed_node_info(
                    peer_info.node_id.key,
                    peer_info.signed_node_info,
                )?;

                // Get the udp direct dialinfo for the hole punch
                peer_nr.filter_protocols(ProtocolSet::only(ProtocolType::UDP));
                let hole_punch_dial_info_detail = peer_nr
                    .first_filtered_dial_info_detail(Some(RoutingDomain::PublicInternet))
                    .ok_or_else(|| "No hole punch capable dialinfo found for node".to_owned())?;

                // Now that we picked a specific dialinfo, further restrict the noderef to the specific address type
                let mut filter = peer_nr.take_filter().unwrap();
                filter.peer_scope = PeerScope::Global;
                filter.address_type = Some(hole_punch_dial_info_detail.dial_info.address_type());
                peer_nr.set_filter(Some(filter));

                // Do our half of the hole punch by sending an empty packet
                // Both sides will do this and then the receipt will get sent over the punched hole
                self.net()
                    .send_data_to_dial_info(
                        hole_punch_dial_info_detail.dial_info.clone(),
                        Vec::new(),
                    )
                    .await?;

                // XXX: do we need a delay here? or another hole punch packet?

                // Return the receipt using the same dial info send the receipt to it
                rpc.rpc_call_return_receipt(Destination::Direct(peer_nr), None, receipt)
                    .await
                    .map_err(map_to_string)?;
            }
        }

        Ok(())
    }

    // Builds an envelope for sending over the network
    #[instrument(level = "trace", skip(self, body), err)]
    fn build_envelope<B: AsRef<[u8]>>(
        &self,
        dest_node_id: DHTKey,
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
    // node_ref is the direct destination to which the envelope will be sent
    // If 'node_id' is specified, it can be different than node_ref.node_id()
    // which will cause the envelope to be relayed
    #[instrument(level = "trace", skip(self, body), ret, err)]
    pub async fn send_envelope<B: AsRef<[u8]>>(
        &self,
        node_ref: NodeRef,
        envelope_node_id: Option<DHTKey>,
        body: B,
    ) -> Result<SendDataKind, String> {
        let via_node_id = node_ref.node_id();
        let envelope_node_id = envelope_node_id.unwrap_or(via_node_id);

        if envelope_node_id != via_node_id {
            log_net!(
                "sending envelope to {:?} via {:?}",
                envelope_node_id,
                node_ref
            );
        } else {
            log_net!("sending envelope to {:?}", node_ref);
        }
        // Get node's min/max version and see if we can send to it
        // and if so, get the max version we can use
        let version = if let Some((node_min, node_max)) = node_ref.operate(|e| e.min_max_version())
        {
            #[allow(clippy::absurd_extreme_comparisons)]
            if node_min > MAX_VERSION || node_max < MIN_VERSION {
                return Err(format!(
                    "can't talk to this node {} because version is unsupported: ({},{})",
                    via_node_id, node_min, node_max
                ))
                .map_err(logthru_rpc!(warn));
            }
            cmp::min(node_max, MAX_VERSION)
        } else {
            MAX_VERSION
        };

        // Build the envelope to send
        let out = self
            .build_envelope(envelope_node_id, version, body)
            .map_err(logthru_rpc!(error))?;

        // Send the envelope via whatever means necessary
        let send_data_kind = self.send_data(node_ref.clone(), out).await?;

        // If we asked to relay from the start, then this is always indirect
        if envelope_node_id != via_node_id {
            return Ok(SendDataKind::GlobalIndirect);
        }
        Ok(send_data_kind)
    }

    // Called by the RPC handler when we want to issue an direct receipt
    #[instrument(level = "trace", skip(self, rcpt_data), err)]
    pub async fn send_out_of_band_receipt(
        &self,
        dial_info: DialInfo,
        rcpt_data: Vec<u8>,
    ) -> Result<(), String> {
        // Do we need to validate the outgoing receipt? Probably not
        // because it is supposed to be opaque and the
        // recipient/originator does the validation
        // Also, in the case of an old 'version', returning the receipt
        // should not be subject to our ability to decode it

        // Send receipt directly
        self.net()
            .send_data_unbound_to_dial_info(dial_info, rcpt_data)
            .await
    }

    // Figure out how to reach a node
    #[instrument(level = "trace", skip(self), ret, err)]
    fn get_contact_method(&self, mut target_node_ref: NodeRef) -> Result<ContactMethod, String> {
        let routing_table = self.routing_table();

        // Get our network class and protocol config
        let our_network_class = self.get_network_class().unwrap_or(NetworkClass::Invalid);
        let our_protocol_config = self.get_protocol_config().unwrap();

        // Scope noderef down to protocols we can do outbound
        if !target_node_ref.filter_protocols(our_protocol_config.outbound) {
            return Ok(ContactMethod::Unreachable);
        }

        // Get the best matching local direct dial info if we have it
        let opt_target_local_did =
            target_node_ref.first_filtered_dial_info_detail(Some(RoutingDomain::LocalNetwork));
        if let Some(target_local_did) = opt_target_local_did {
            return Ok(ContactMethod::Direct(target_local_did.dial_info));
        }

        // Get the best match internet dial info if we have it
        let opt_target_public_did =
            target_node_ref.first_filtered_dial_info_detail(Some(RoutingDomain::PublicInternet));
        if let Some(target_public_did) = opt_target_public_did {
            // Do we need to signal before going inbound?
            if !target_public_did.class.requires_signal() {
                // Go direct without signaling
                return Ok(ContactMethod::Direct(target_public_did.dial_info));
            }

            // Get the target's inbound relay, it must have one or it is not reachable
            if let Some(inbound_relay_nr) = target_node_ref.relay() {
                // Can we reach the inbound relay?
                if inbound_relay_nr
                    .first_filtered_dial_info_detail(Some(RoutingDomain::PublicInternet))
                    .is_some()
                {
                    // Can we receive anything inbound ever?
                    if matches!(our_network_class, NetworkClass::InboundCapable) {
                        // Get the best match dial info for an reverse inbound connection
                        let reverse_dif = DialInfoFilter::global().with_protocol_set(
                            target_node_ref.outbound_protocols().unwrap_or_default(),
                        );
                        if let Some(reverse_did) = routing_table.first_filtered_dial_info_detail(
                            Some(RoutingDomain::PublicInternet),
                            &reverse_dif,
                        ) {
                            // Can we receive a direct reverse connection?
                            if !reverse_did.class.requires_signal() {
                                return Ok(ContactMethod::SignalReverse(
                                    inbound_relay_nr,
                                    target_node_ref,
                                ));
                            }
                        }

                        // Does we and the target have outbound protocols to hole-punch?
                        if our_protocol_config.outbound.contains(ProtocolType::UDP)
                            && target_node_ref
                                .outbound_protocols()
                                .unwrap_or_default()
                                .contains(ProtocolType::UDP)
                        {
                            // Do the target and self nodes have a direct udp dialinfo
                            let udp_dif =
                                DialInfoFilter::global().with_protocol_type(ProtocolType::UDP);
                            let mut udp_target_nr = target_node_ref.clone();
                            udp_target_nr.filter_protocols(ProtocolSet::only(ProtocolType::UDP));
                            let target_has_udp_dialinfo = target_node_ref
                                .first_filtered_dial_info_detail(Some(
                                    RoutingDomain::PublicInternet,
                                ))
                                .is_some();
                            let self_has_udp_dialinfo = routing_table
                                .first_filtered_dial_info_detail(
                                    Some(RoutingDomain::PublicInternet),
                                    &udp_dif,
                                )
                                .is_some();
                            if target_has_udp_dialinfo && self_has_udp_dialinfo {
                                return Ok(ContactMethod::SignalHolePunch(
                                    inbound_relay_nr,
                                    udp_target_nr,
                                ));
                            }
                        }
                        // Otherwise we have to inbound relay
                    }

                    return Ok(ContactMethod::InboundRelay(inbound_relay_nr));
                }
            }
        }
        // If the other node is not inbound capable at all, it is using a full relay
        else if let Some(target_inbound_relay_nr) = target_node_ref.relay() {
            // Can we reach the full relay?
            if target_inbound_relay_nr
                .first_filtered_dial_info_detail(Some(RoutingDomain::PublicInternet))
                .is_some()
            {
                return Ok(ContactMethod::InboundRelay(target_inbound_relay_nr));
            }
        }

        // If we can't reach the node by other means, try our outbound relay if we have one
        if let Some(relay_node) = self.relay_node() {
            return Ok(ContactMethod::OutboundRelay(relay_node));
        }
        // Otherwise, we can't reach this node
        debug!("unable to reach node {:?}", target_node_ref);
        // trace!(
        //     "unable to reach node {:?}: {}",
        //     target_node_ref,
        //     target_node_ref.operate(|e| format!("{:#?}", e))
        // );
        Ok(ContactMethod::Unreachable)
    }

    // Send a reverse connection signal and wait for the return receipt over it
    // Then send the data across the new connection
    #[instrument(level = "trace", skip(self, data), err)]
    pub async fn do_reverse_connect(
        &self,
        relay_nr: NodeRef,
        target_nr: NodeRef,
        data: Vec<u8>,
    ) -> Result<(), String> {
        // Build a return receipt for the signal
        let receipt_timeout =
            ms_to_us(self.config.get().network.reverse_connection_receipt_time_ms);
        let (receipt, eventual_value) = self
            .generate_single_shot_receipt(receipt_timeout, [])
            .map_err(map_to_string)?;

        // Get our peer info
        let peer_info = self.routing_table().get_own_peer_info();

        // Issue the signal
        let rpc = self.rpc_processor();
        rpc.rpc_call_signal(
            Destination::Relay(relay_nr.clone(), target_nr.node_id()),
            None,
            SignalInfo::ReverseConnect { receipt, peer_info },
        )
        .await
        .map_err(logthru_net!("failed to send signal to {:?}", relay_nr))
        .map_err(map_to_string)?;
        // Wait for the return receipt
        let inbound_nr = match eventual_value.await.take_value().unwrap() {
            ReceiptEvent::ReturnedOutOfBand => {
                return Err("reverse connect receipt should be returned in-band".to_owned());
            }
            ReceiptEvent::ReturnedInBand { inbound_noderef } => inbound_noderef,
            ReceiptEvent::Expired => {
                return Err(format!(
                    "reverse connect receipt expired from {:?}",
                    target_nr
                ));
            }
            ReceiptEvent::Cancelled => {
                return Err(format!(
                    "reverse connect receipt cancelled from {:?}",
                    target_nr
                ));
            }
        };

        // We expect the inbound noderef to be the same as the target noderef
        // if they aren't the same, we should error on this and figure out what then hell is up
        if target_nr != inbound_nr {
            error!("unexpected noderef mismatch on reverse connect");
        }

        // And now use the existing connection to send over
        if let Some(descriptor) = inbound_nr.last_connection().await {
            match self
                .net()
                .send_data_to_existing_connection(descriptor, data)
                .await
                .map_err(logthru_net!())?
            {
                None => Ok(()),
                Some(_) => Err("unable to send over reverse connection".to_owned()),
            }
        } else {
            Err("no reverse connection available".to_owned())
        }
    }

    // Send a hole punch signal and do a negotiating ping and wait for the return receipt
    // Then send the data across the new connection
    #[instrument(level = "trace", skip(self, data), err)]
    pub async fn do_hole_punch(
        &self,
        relay_nr: NodeRef,
        target_nr: NodeRef,
        data: Vec<u8>,
    ) -> Result<(), String> {
        // Ensure we are filtered down to UDP (the only hole punch protocol supported today)
        assert!(relay_nr
            .filter_ref()
            .map(|dif| dif.protocol_set == ProtocolSet::only(ProtocolType::UDP))
            .unwrap_or_default());
        assert!(target_nr
            .filter_ref()
            .map(|dif| dif.protocol_set == ProtocolSet::only(ProtocolType::UDP))
            .unwrap_or_default());

        // Build a return receipt for the signal
        let receipt_timeout =
            ms_to_us(self.config.get().network.reverse_connection_receipt_time_ms);
        let (receipt, eventual_value) = self
            .generate_single_shot_receipt(receipt_timeout, [])
            .map_err(map_to_string)?;

        // Get our peer info
        let peer_info = self.routing_table().get_own_peer_info();

        // Get the udp direct dialinfo for the hole punch
        let hole_punch_did = target_nr
            .first_filtered_dial_info_detail(Some(RoutingDomain::PublicInternet))
            .ok_or_else(|| "No hole punch capable dialinfo found for node".to_owned())?;

        // Do our half of the hole punch by sending an empty packet
        // Both sides will do this and then the receipt will get sent over the punched hole
        self.net()
            .send_data_to_dial_info(hole_punch_did.dial_info, Vec::new())
            .await?;

        // Issue the signal
        let rpc = self.rpc_processor();
        rpc.rpc_call_signal(
            Destination::Relay(relay_nr.clone(), target_nr.node_id()),
            None,
            SignalInfo::HolePunch { receipt, peer_info },
        )
        .await
        .map_err(logthru_net!("failed to send signal to {:?}", relay_nr))
        .map_err(map_to_string)?;

        // Wait for the return receipt
        let inbound_nr = match eventual_value.await.take_value().unwrap() {
            ReceiptEvent::ReturnedOutOfBand => {
                return Err("hole punch receipt should be returned in-band".to_owned());
            }
            ReceiptEvent::ReturnedInBand { inbound_noderef } => inbound_noderef,
            ReceiptEvent::Expired => {
                return Err(format!("hole punch receipt expired from {}", target_nr));
            }
            ReceiptEvent::Cancelled => {
                return Err(format!("hole punch receipt cancelled from {}", target_nr));
            }
        };

        // We expect the inbound noderef to be the same as the target noderef
        // if they aren't the same, we should error on this and figure out what then hell is up
        if target_nr != inbound_nr {
            return Err(format!(
                "unexpected noderef mismatch on hole punch {}, expected {}",
                inbound_nr, target_nr
            ));
        }

        // And now use the existing connection to send over
        if let Some(descriptor) = inbound_nr.last_connection().await {
            match self
                .net()
                .send_data_to_existing_connection(descriptor, data)
                .await
                .map_err(logthru_net!())?
            {
                None => Ok(()),
                Some(_) => Err("unable to send over hole punch".to_owned()),
            }
        } else {
            Err("no hole punch available".to_owned())
        }
    }

    // Send raw data to a node
    //
    // We may not have dial info for a node, but have an existing connection for it
    // because an inbound connection happened first, and no FindNodeQ has happened to that
    // node yet to discover its dial info. The existing connection should be tried first
    // in this case.
    //
    // Sending to a node requires determining a NetworkClass compatible mechanism
    //
    pub fn send_data(
        &self,
        node_ref: NodeRef,
        data: Vec<u8>,
    ) -> SystemPinBoxFuture<Result<SendDataKind, String>> {
        let this = self.clone();
        Box::pin(async move {
            // First try to send data to the last socket we've seen this peer on
            let data = if let Some(descriptor) = node_ref.last_connection().await {
                match this
                    .net()
                    .send_data_to_existing_connection(descriptor, data)
                    .await
                    .map_err(logthru_net!())?
                {
                    None => {
                        return Ok(if descriptor.matches_peer_scope(PeerScope::Local) {
                            SendDataKind::LocalDirect
                        } else {
                            SendDataKind::GlobalDirect
                        });
                    }
                    Some(d) => d,
                }
            } else {
                data
            };

            log_net!("send_data via dialinfo to {:?}", node_ref);
            // If we don't have last_connection, try to reach out to the peer via its dial info
            match this
                .get_contact_method(node_ref.clone())
                .map_err(logthru_net!(debug))
                .map(logthru_net!("get_contact_method for {:?}", node_ref))?
            {
                ContactMethod::OutboundRelay(relay_nr) | ContactMethod::InboundRelay(relay_nr) => {
                    this.send_data(relay_nr, data)
                        .await
                        .map(|_| SendDataKind::GlobalIndirect)
                }
                ContactMethod::Direct(dial_info) => {
                    let send_data_kind = if dial_info.is_local() {
                        SendDataKind::LocalDirect
                    } else {
                        SendDataKind::GlobalDirect
                    };
                    this.net()
                        .send_data_to_dial_info(dial_info, data)
                        .await
                        .map(|_| send_data_kind)
                }
                ContactMethod::SignalReverse(relay_nr, target_node_ref) => this
                    .do_reverse_connect(relay_nr, target_node_ref, data)
                    .await
                    .map(|_| SendDataKind::GlobalDirect),
                ContactMethod::SignalHolePunch(relay_nr, target_node_ref) => this
                    .do_hole_punch(relay_nr, target_node_ref, data)
                    .await
                    .map(|_| SendDataKind::GlobalDirect),
                ContactMethod::Unreachable => Err("Can't send to this node".to_owned()),
            }
            .map_err(logthru_net!(debug))
        })
    }

    // Called when a packet potentially containing an RPC envelope is received by a low-level
    // network protocol handler. Processes the envelope, authenticates and decrypts the RPC message
    // and passes it to the RPC handler
    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    async fn on_recv_envelope(
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
        self.stats_packet_rcvd(descriptor.remote_address().to_ip_addr(), data.len() as u64);

        // Ensure we can read the magic number
        if data.len() < 4 {
            return Err("short packet".to_owned());
        }

        // Is this an out-of-band receipt instead of an envelope?
        if data[0..4] == *RECEIPT_MAGIC {
            self.handle_out_of_band_receipt(data).await?;
            return Ok(true);
        }

        // Decode envelope header (may fail signature validation)
        let envelope = Envelope::from_signed_data(data).map_err(|_| {
            format!(
                "envelope failed to decode from {:?}: {} bytes",
                descriptor,
                data.len()
            )
        })?;

        // Get routing table and rpc processor
        let (routing_table, rpc) = {
            let inner = self.inner.lock();
            (
                inner.routing_table.as_ref().unwrap().clone(),
                inner.components.as_ref().unwrap().rpc_processor.clone(),
            )
        };

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

        // Peek at header and see if we need to relay this
        // If the recipient id is not our node id, then it needs relaying
        let sender_id = envelope.get_sender_id();
        let recipient_id = envelope.get_recipient_id();
        if recipient_id != routing_table.node_id() {
            // See if the source node is allowed to resolve nodes
            // This is a costly operation, so only outbound-relay permitted
            // nodes are allowed to do this, for example PWA users

            let relay_nr = if self.check_client_whitelist(sender_id) {
                // Full relay allowed, do a full resolve_node
                rpc.resolve_node(recipient_id).await.map_err(|e| {
                    format!(
                        "failed to resolve recipient node for relay, dropping outbound relayed packet...: {:?}",
                        e
                    )
                }).map_err(logthru_net!())?
            } else {
                // If this is not a node in the client whitelist, only allow inbound relay
                // which only performs a lightweight lookup before passing the packet back out

                // See if we have the node in our routing table
                // We should, because relays are chosen by nodes that have established connectivity and
                // should be mutually in each others routing tables. The node needing the relay will be
                // pinging this node regularly to keep itself in the routing table
                routing_table.lookup_node_ref(recipient_id).ok_or_else(|| {
                    format!(
                        "Inbound relay asked for recipient not in routing table: {}",
                        recipient_id
                    )
                })?
            };

            // Relay the packet to the desired destination
            self.send_data(relay_nr, data.to_vec())
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

    // Keep relays assigned and accessible
    #[instrument(level = "trace", skip(self), err)]
    async fn relay_management_task_routine(self, _last_ts: u64, cur_ts: u64) -> Result<(), String> {
        // log_net!("--- network manager relay_management task");

        // Get our node's current node info and network class and do the right thing
        let routing_table = self.routing_table();
        let node_info = routing_table.get_own_node_info();
        let network_class = self.get_network_class();

        // Do we know our network class yet?
        if let Some(network_class) = network_class {
            // If we already have a relay, see if it is dead, or if we don't need it any more
            {
                let mut inner = self.inner.lock();
                if let Some(relay_node) = inner.relay_node.clone() {
                    let state = relay_node.operate(|e| e.state(cur_ts));
                    if matches!(state, BucketEntryState::Dead) || !node_info.requires_relay() {
                        // Relay node is dead or no longer needed
                        inner.relay_node = None;
                    }
                }
            }

            // Do we need a relay?
            if node_info.requires_relay() {
                // Do we need an outbound relay?
                if network_class.outbound_wants_relay() {
                    // The outbound relay is the host of the PWA
                    if let Some(outbound_relay_peerinfo) = intf::get_outbound_relay_peer().await {
                        let mut inner = self.inner.lock();

                        // Register new outbound relay
                        let nr = routing_table.register_node_with_signed_node_info(
                            outbound_relay_peerinfo.node_id.key,
                            outbound_relay_peerinfo.signed_node_info,
                        )?;
                        inner.relay_node = Some(nr);
                    }
                // Otherwise we must need an inbound relay
                } else {
                    // Find a node in our routing table that is an acceptable inbound relay
                    if let Some(nr) = routing_table.find_inbound_relay(cur_ts) {
                        let mut inner = self.inner.lock();
                        inner.relay_node = Some(nr);
                    }
                }
            }
        }

        Ok(())
    }

    // Compute transfer statistics for the low level network
    #[instrument(level = "trace", skip(self), err)]
    async fn rolling_transfers_task_routine(self, last_ts: u64, cur_ts: u64) -> Result<(), String> {
        // log_net!("--- network manager rolling_transfers task");
        {
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
        }

        // Send update
        self.send_network_update();

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

    // Get stats
    pub fn get_stats(&self) -> NetworkManagerStats {
        let inner = self.inner.lock();
        inner.stats.clone()
    }

    fn get_veilid_state_inner(inner: &NetworkManagerInner) -> VeilidStateNetwork {
        if inner.components.is_some() && inner.components.as_ref().unwrap().net.is_started() {
            VeilidStateNetwork {
                started: true,
                bps_down: inner.stats.self_stats.transfer_stats.down.average,
                bps_up: inner.stats.self_stats.transfer_stats.up.average,
            }
        } else {
            VeilidStateNetwork {
                started: false,
                bps_down: 0,
                bps_up: 0,
            }
        }
    }
    pub fn get_veilid_state(&self) -> VeilidStateNetwork {
        let inner = self.inner.lock();
        Self::get_veilid_state_inner(&*inner)
    }

    fn send_network_update(&self) {
        let (update_cb, state) = {
            let inner = self.inner.lock();
            let update_cb = inner.update_callback.clone();
            if update_cb.is_none() {
                return;
            }
            let state = Self::get_veilid_state_inner(&*inner);
            (update_cb.unwrap(), state)
        };
        update_cb(VeilidUpdate::Network(state));
    }

    // Determine if a local IP address has changed
    // this means we should restart the low level network and and recreate all of our dial info
    // Wait until we have received confirmation from N different peers
    pub async fn report_local_socket_address(
        &self,
        _socket_address: SocketAddress,
        _reporting_peer: NodeRef,
    ) {
        // XXX: Nothing here yet.
    }

    // Determine if a global IP address has changed
    // this means we should recreate our public dial info if it is not static and rediscover it
    // Wait until we have received confirmation from N different peers
    pub async fn report_global_socket_address(
        &self,
        socket_address: SocketAddress,
        reporting_peer: NodeRef,
    ) {
        let (net, routing_table) = {
            let mut inner = self.inner.lock();

            // Store the reported address
            inner
                .public_address_check_cache
                .insert(reporting_peer.node_id(), socket_address);

            let net = inner.components.as_ref().unwrap().net.clone();
            let routing_table = inner.routing_table.as_ref().unwrap().clone();
            (net, routing_table)
        };
        let network_class = net.get_network_class().unwrap_or(NetworkClass::Invalid);

        // Determine if our external address has likely changed
        let needs_public_address_detection =
            if matches!(network_class, NetworkClass::InboundCapable) {
                // Get current external ip/port from registered global dialinfo
                let current_addresses: BTreeSet<SocketAddress> = routing_table
                    .all_filtered_dial_info_details(
                        Some(RoutingDomain::PublicInternet),
                        &DialInfoFilter::all(),
                    )
                    .iter()
                    .map(|did| did.dial_info.socket_address())
                    .collect();

                // If we are inbound capable, but start to see inconsistent socket addresses from multiple reporting peers
                // then we zap the network class and re-detect it
                let inner = self.inner.lock();
                let mut inconsistencies = 0;
                let mut changed = false;
                // Iteration goes from most recent to least recent node/address pair
                for (_, a) in &inner.public_address_check_cache {
                    if !current_addresses.contains(a) {
                        inconsistencies += 1;
                        if inconsistencies >= GLOBAL_ADDRESS_CHANGE_DETECTION_COUNT {
                            changed = true;
                            break;
                        }
                    }
                }
                changed
            } else {
                // If we are currently outbound only, we don't have any public dial info
                // but if we are starting to see consistent socket address from multiple reporting peers
                // then we may be become inbound capable, so zap the network class so we can re-detect it and any public dial info

                let inner = self.inner.lock();
                let mut consistencies = 0;
                let mut consistent = false;
                let mut current_address = Option::<SocketAddress>::None;
                // Iteration goes from most recent to least recent node/address pair
                for (_, a) in &inner.public_address_check_cache {
                    if let Some(current_address) = current_address {
                        if current_address == *a {
                            consistencies += 1;
                            if consistencies >= GLOBAL_ADDRESS_CHANGE_DETECTION_COUNT {
                                consistent = true;
                                break;
                            }
                        }
                    } else {
                        current_address = Some(*a);
                    }
                }
                consistent
            };

        if needs_public_address_detection {
            // Reset the address check cache now so we can start detecting fresh
            let mut inner = self.inner.lock();
            inner.public_address_check_cache.clear();

            // Reset the network class and dial info so we can re-detect it
            routing_table.clear_dial_info_details(RoutingDomain::PublicInternet);
            net.reset_network_class();
        }
    }
}
