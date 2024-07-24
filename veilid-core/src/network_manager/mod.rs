use crate::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

mod address_filter;
mod connection_handle;
mod connection_manager;
mod connection_table;
mod direct_boot;
mod network_connection;
mod receipt_manager;
mod send_data;
mod stats;
mod tasks;
mod types;

#[doc(hidden)]
pub mod tests;

////////////////////////////////////////////////////////////////////////////////////////

pub(crate) use connection_manager::*;
pub(crate) use network_connection::*;
pub(crate) use receipt_manager::*;
pub(crate) use stats::*;

pub use types::*;

////////////////////////////////////////////////////////////////////////////////////////
use address_filter::*;
use connection_handle::*;
use crypto::*;
use futures_util::stream::FuturesUnordered;
use hashlink::LruCache;
#[cfg(not(target_arch = "wasm32"))]
use native::*;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{MAX_CAPABILITIES, PUBLIC_INTERNET_CAPABILITIES};
use routing_table::*;
use rpc_processor::*;
use storage_manager::*;
#[cfg(target_arch = "wasm32")]
use wasm::*;
#[cfg(target_arch = "wasm32")]
pub use wasm::{/* LOCAL_NETWORK_CAPABILITIES, */ MAX_CAPABILITIES, PUBLIC_INTERNET_CAPABILITIES,};

////////////////////////////////////////////////////////////////////////////////////////

pub const MAX_MESSAGE_SIZE: usize = MAX_ENVELOPE_SIZE;
pub const IPADDR_TABLE_SIZE: usize = 1024;
pub const IPADDR_MAX_INACTIVE_DURATION_US: TimestampDuration =
    TimestampDuration::new(300_000_000u64); // 5 minutes
pub const NODE_CONTACT_METHOD_CACHE_SIZE: usize = 1024;
pub const PUBLIC_ADDRESS_CHANGE_DETECTION_COUNT: usize = 5;
pub const PUBLIC_ADDRESS_CHECK_CACHE_SIZE: usize = 10;
pub const PUBLIC_ADDRESS_CHECK_TASK_INTERVAL_SECS: u32 = 60;
pub const PUBLIC_ADDRESS_INCONSISTENCY_TIMEOUT_US: TimestampDuration =
    TimestampDuration::new(300_000_000u64); // 5 minutes
pub const PUBLIC_ADDRESS_INCONSISTENCY_PUNISHMENT_TIMEOUT_US: TimestampDuration =
    TimestampDuration::new(3_600_000_000_u64); // 60 minutes
pub const ADDRESS_FILTER_TASK_INTERVAL_SECS: u32 = 60;
pub const BOOT_MAGIC: &[u8; 4] = b"BOOT";

#[derive(Clone, Debug, Default)]
pub struct ProtocolConfig {
    pub outbound: ProtocolTypeSet,
    pub inbound: ProtocolTypeSet,
    pub family_global: AddressTypeSet,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub family_local: AddressTypeSet,
    pub public_internet_capabilities: Vec<FourCC>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub local_network_capabilities: Vec<FourCC>,
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

#[derive(Debug)]
struct ClientAllowlistEntry {
    last_seen_ts: Timestamp,
}

#[derive(Clone, Debug)]
pub(crate) struct SendDataMethod {
    /// How the data was sent, possibly to a relay
    pub contact_method: NodeContactMethod,
    /// Pre-relayed contact method
    pub opt_relayed_contact_method: Option<NodeContactMethod>,
    /// The specific flow used to send the data
    pub unique_flow: UniqueFlow,
}

/// Mechanism required to contact another node
#[derive(Clone, Debug)]
pub(crate) enum NodeContactMethod {
    /// Node is not reachable by any means
    Unreachable,
    /// Connection should have already existed
    Existing,
    /// Contact the node directly
    Direct(DialInfo),
    /// Request via signal the node connect back directly (relay, target)
    SignalReverse(NodeRef, NodeRef),
    /// Request via signal the node negotiate a hole punch (relay, target)
    SignalHolePunch(NodeRef, NodeRef),
    /// Must use an inbound relay to reach the node
    InboundRelay(NodeRef),
    /// Must use outbound relay to reach the node
    OutboundRelay(NodeRef),
}
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
struct NodeContactMethodCacheKey {
    node_ids: TypedKeyGroup,
    own_node_info_ts: Timestamp,
    target_node_info_ts: Timestamp,
    target_node_ref_filter: Option<NodeRefFilter>,
    target_node_ref_sequencing: Sequencing,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
struct PublicAddressCheckCacheKey(ProtocolType, AddressType);

enum SendDataToExistingFlowResult {
    Sent(UniqueFlow),
    NotSent(Vec<u8>),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StartupDisposition {
    Success,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    BindRetry,
}

// The mutable state of the network manager
struct NetworkManagerInner {
    stats: NetworkManagerStats,
    client_allowlist: LruCache<TypedKey, ClientAllowlistEntry>,
    node_contact_method_cache: LruCache<NodeContactMethodCacheKey, NodeContactMethod>,
    public_address_check_cache:
        BTreeMap<PublicAddressCheckCacheKey, LruCache<IpAddr, SocketAddress>>,
    public_address_inconsistencies_table:
        BTreeMap<PublicAddressCheckCacheKey, HashMap<IpAddr, Timestamp>>,
}

struct NetworkManagerUnlockedInner {
    // Handles
    config: VeilidConfig,
    storage_manager: StorageManager,
    table_store: TableStore,
    #[cfg(feature = "unstable-blockstore")]
    block_store: BlockStore,
    crypto: Crypto,
    // Accessors
    routing_table: RwLock<Option<RoutingTable>>,
    address_filter: RwLock<Option<AddressFilter>>,
    components: RwLock<Option<NetworkComponents>>,
    update_callback: RwLock<Option<UpdateCallback>>,
    // Background processes
    rolling_transfers_task: TickTask<EyreReport>,
    public_address_check_task: TickTask<EyreReport>,
    address_filter_task: TickTask<EyreReport>,
    // Network Key
    network_key: Option<SharedSecret>,
    // Startup Lock
    startup_lock: StartupLock,
}

#[derive(Clone)]
pub(crate) struct NetworkManager {
    inner: Arc<Mutex<NetworkManagerInner>>,
    unlocked_inner: Arc<NetworkManagerUnlockedInner>,
}

impl NetworkManager {
    fn new_inner() -> NetworkManagerInner {
        NetworkManagerInner {
            stats: NetworkManagerStats::default(),
            client_allowlist: LruCache::new_unbounded(),
            node_contact_method_cache: LruCache::new(NODE_CONTACT_METHOD_CACHE_SIZE),
            public_address_check_cache: BTreeMap::new(),
            public_address_inconsistencies_table: BTreeMap::new(),
        }
    }
    fn new_unlocked_inner(
        config: VeilidConfig,
        storage_manager: StorageManager,
        table_store: TableStore,
        #[cfg(feature = "unstable-blockstore")] block_store: BlockStore,
        crypto: Crypto,
        network_key: Option<SharedSecret>,
    ) -> NetworkManagerUnlockedInner {
        NetworkManagerUnlockedInner {
            config: config.clone(),
            storage_manager,
            table_store,
            #[cfg(feature = "unstable-blockstore")]
            block_store,
            crypto,
            address_filter: RwLock::new(None),
            routing_table: RwLock::new(None),
            components: RwLock::new(None),
            update_callback: RwLock::new(None),
            rolling_transfers_task: TickTask::new(
                "rolling_transfers_task",
                ROLLING_TRANSFERS_INTERVAL_SECS,
            ),
            public_address_check_task: TickTask::new(
                "public_address_check_task",
                PUBLIC_ADDRESS_CHECK_TASK_INTERVAL_SECS,
            ),
            address_filter_task: TickTask::new(
                "address_filter_task",
                ADDRESS_FILTER_TASK_INTERVAL_SECS,
            ),
            network_key,
            startup_lock: StartupLock::new(),
        }
    }

    pub fn new(
        config: VeilidConfig,
        storage_manager: StorageManager,
        table_store: TableStore,
        #[cfg(feature = "unstable-blockstore")] block_store: BlockStore,
        crypto: Crypto,
    ) -> Self {
        // Make the network key
        let network_key = {
            let c = config.get();
            let network_key_password = c.network.network_key_password.clone();
            let network_key = if let Some(network_key_password) = network_key_password {
                if !network_key_password.is_empty() {
                    info!("Using network key");

                    let bcs = crypto.best();
                    // Yes the use of the salt this way is generally bad, but this just needs to be hashed
                    Some(
                        bcs.derive_shared_secret(
                            network_key_password.as_bytes(),
                            &bcs.generate_hash(network_key_password.as_bytes()).bytes,
                        )
                        .expect("failed to derive network key"),
                    )
                } else {
                    None
                }
            } else {
                None
            };

            network_key
        };

        let this = Self {
            inner: Arc::new(Mutex::new(Self::new_inner())),
            unlocked_inner: Arc::new(Self::new_unlocked_inner(
                config,
                storage_manager,
                table_store,
                #[cfg(feature = "unstable-blockstore")]
                block_store,
                crypto,
                network_key,
            )),
        };

        this.setup_tasks();

        this
    }
    pub fn config(&self) -> VeilidConfig {
        self.unlocked_inner.config.clone()
    }
    pub fn with_config<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&VeilidConfigInner) -> R,
    {
        f(&self.unlocked_inner.config.get())
    }
    pub fn storage_manager(&self) -> StorageManager {
        self.unlocked_inner.storage_manager.clone()
    }
    pub fn table_store(&self) -> TableStore {
        self.unlocked_inner.table_store.clone()
    }
    #[cfg(feature = "unstable-blockstore")]
    pub fn block_store(&self) -> BlockStore {
        self.unlocked_inner.block_store.clone()
    }
    pub fn crypto(&self) -> Crypto {
        self.unlocked_inner.crypto.clone()
    }
    pub fn address_filter(&self) -> AddressFilter {
        self.unlocked_inner
            .address_filter
            .read()
            .as_ref()
            .unwrap()
            .clone()
    }
    pub fn routing_table(&self) -> RoutingTable {
        self.unlocked_inner
            .routing_table
            .read()
            .as_ref()
            .unwrap()
            .clone()
    }
    fn net(&self) -> Network {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .unwrap()
            .net
            .clone()
    }
    fn opt_net(&self) -> Option<Network> {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .map(|x| x.net.clone())
    }
    fn receipt_manager(&self) -> ReceiptManager {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .unwrap()
            .receipt_manager
            .clone()
    }
    pub fn rpc_processor(&self) -> RPCProcessor {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .unwrap()
            .rpc_processor
            .clone()
    }
    pub fn opt_rpc_processor(&self) -> Option<RPCProcessor> {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .map(|x| x.rpc_processor.clone())
    }

    pub fn connection_manager(&self) -> ConnectionManager {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .unwrap()
            .connection_manager
            .clone()
    }
    pub fn opt_connection_manager(&self) -> Option<ConnectionManager> {
        self.unlocked_inner
            .components
            .read()
            .as_ref()
            .map(|x| x.connection_manager.clone())
    }

    pub fn update_callback(&self) -> UpdateCallback {
        self.unlocked_inner
            .update_callback
            .read()
            .as_ref()
            .unwrap()
            .clone()
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn init(&self, update_callback: UpdateCallback) -> EyreResult<()> {
        let routing_table = RoutingTable::new(self.clone());
        routing_table.init().await?;
        let address_filter = AddressFilter::new(self.config(), routing_table.clone());
        *self.unlocked_inner.routing_table.write() = Some(routing_table.clone());
        *self.unlocked_inner.address_filter.write() = Some(address_filter);
        *self.unlocked_inner.update_callback.write() = Some(update_callback);
        Ok(())
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn terminate(&self) {
        let routing_table = self.unlocked_inner.routing_table.write().take();
        if let Some(routing_table) = routing_table {
            routing_table.terminate().await;
        }
        *self.unlocked_inner.update_callback.write() = None;
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn internal_startup(&self) -> EyreResult<StartupDisposition> {
        if self.unlocked_inner.components.read().is_some() {
            log_net!(debug "NetworkManager::internal_startup already started");
            return Ok(StartupDisposition::Success);
        }

        // Clean address filter for things that should not be persistent
        self.address_filter().restart();

        // Create network components
        let connection_manager = ConnectionManager::new(self.clone());
        let net = Network::new(
            self.clone(),
            self.routing_table(),
            connection_manager.clone(),
        );
        let rpc_processor = RPCProcessor::new(
            self.clone(),
            self.unlocked_inner
                .update_callback
                .read()
                .as_ref()
                .unwrap()
                .clone(),
        );
        let receipt_manager = ReceiptManager::new(self.clone());
        *self.unlocked_inner.components.write() = Some(NetworkComponents {
            net: net.clone(),
            connection_manager: connection_manager.clone(),
            rpc_processor: rpc_processor.clone(),
            receipt_manager: receipt_manager.clone(),
        });

        // Start network components
        connection_manager.startup().await?;
        match net.startup().await? {
            StartupDisposition::Success => {}
            StartupDisposition::BindRetry => {
                return Ok(StartupDisposition::BindRetry);
            }
        }
        rpc_processor.startup().await?;
        receipt_manager.startup().await?;

        log_net!("NetworkManager::internal_startup end");

        Ok(StartupDisposition::Success)
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn startup(&self) -> EyreResult<StartupDisposition> {
        let guard = self.unlocked_inner.startup_lock.startup()?;

        match self.internal_startup().await {
            Ok(StartupDisposition::Success) => {
                guard.success();

                // Inform api clients that things have changed
                self.send_network_update();

                Ok(StartupDisposition::Success)
            }
            Ok(StartupDisposition::BindRetry) => {
                self.shutdown_internal().await;
                Ok(StartupDisposition::BindRetry)
            }
            Err(e) => {
                self.shutdown_internal().await;
                Err(e)
            }
        }
    }

    #[instrument(level = "debug", skip_all)]
    async fn shutdown_internal(&self) {
        // Cancel all tasks
        self.cancel_tasks().await;

        // Shutdown network components if they started up
        log_net!(debug "shutting down network components");

        let components = self.unlocked_inner.components.read().clone();
        if let Some(components) = components {
            components.net.shutdown().await;
            components.rpc_processor.shutdown().await;
            components.receipt_manager.shutdown().await;
            components.connection_manager.shutdown().await;

            *self.unlocked_inner.components.write() = None;
        }

        // reset the state
        log_net!(debug "resetting network manager state");
        {
            *self.inner.lock() = NetworkManager::new_inner();
        }
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn shutdown(&self) {
        log_net!(debug "starting network manager shutdown");

        let Ok(guard) = self.unlocked_inner.startup_lock.shutdown().await else {
            log_net!(debug "network manager is already shut down");
            return;
        };

        self.shutdown_internal().await;

        guard.success();

        // send update
        log_net!(debug "sending network state update to api clients");
        self.send_network_update();

        log_net!(debug "finished network manager shutdown");
    }

    pub fn update_client_allowlist(&self, client: TypedKey) {
        let mut inner = self.inner.lock();
        match inner.client_allowlist.entry(client) {
            hashlink::lru_cache::Entry::Occupied(mut entry) => {
                entry.get_mut().last_seen_ts = get_aligned_timestamp()
            }
            hashlink::lru_cache::Entry::Vacant(entry) => {
                entry.insert(ClientAllowlistEntry {
                    last_seen_ts: get_aligned_timestamp(),
                });
            }
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub fn check_client_allowlist(&self, client: TypedKey) -> bool {
        let mut inner = self.inner.lock();

        match inner.client_allowlist.entry(client) {
            hashlink::lru_cache::Entry::Occupied(mut entry) => {
                entry.get_mut().last_seen_ts = get_aligned_timestamp();
                true
            }
            hashlink::lru_cache::Entry::Vacant(_) => false,
        }
    }

    pub fn purge_client_allowlist(&self) {
        let timeout_ms = self.with_config(|c| c.network.client_allowlist_timeout_ms);
        let mut inner = self.inner.lock();
        let cutoff_timestamp =
            get_aligned_timestamp() - TimestampDuration::new((timeout_ms as u64) * 1000u64);
        // Remove clients from the allowlist that haven't been since since our allowlist timeout
        while inner
            .client_allowlist
            .peek_lru()
            .map(|v| v.1.last_seen_ts < cutoff_timestamp)
            .unwrap_or_default()
        {
            let (k, v) = inner.client_allowlist.remove_lru().unwrap();
            trace!(target: "net", key=?k, value=?v, "purge_client_allowlist: remove_lru")
        }
    }

    pub fn network_needs_restart(&self) -> bool {
        self.opt_net()
            .map(|net| net.needs_restart())
            .unwrap_or(false)
    }

    pub fn network_is_started(&self) -> bool {
        self.opt_net().map(|net| net.is_started()).unwrap_or(false)
    }

    pub fn generate_node_status(&self, _routing_domain: RoutingDomain) -> NodeStatus {
        NodeStatus {}
    }

    /// Generates a multi-shot/normal receipt
    #[instrument(level = "trace", skip(self, extra_data, callback))]
    pub fn generate_receipt<D: AsRef<[u8]>>(
        &self,
        expiration_us: u64,
        expected_returns: u32,
        extra_data: D,
        callback: impl ReceiptCallback,
    ) -> EyreResult<Vec<u8>> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            bail!("network is not started");
        };
        let receipt_manager = self.receipt_manager();
        let routing_table = self.routing_table();

        // Generate receipt and serialized form to return
        let vcrypto = self.crypto().best();

        let nonce = vcrypto.random_nonce();
        let node_id = routing_table.node_id(vcrypto.kind());
        let node_id_secret = routing_table.node_id_secret_key(vcrypto.kind());

        let receipt = Receipt::try_new(
            best_envelope_version(),
            node_id.kind,
            nonce,
            node_id.value,
            extra_data,
        )?;
        let out = receipt
            .to_signed_data(self.crypto(), &node_id_secret)
            .wrap_err("failed to generate signed receipt")?;

        // Record the receipt for later
        let exp_ts = get_aligned_timestamp() + expiration_us;
        receipt_manager.record_receipt(receipt, exp_ts, expected_returns, callback);

        Ok(out)
    }

    /// Generates a single-shot/normal receipt
    #[instrument(level = "trace", skip(self, extra_data))]
    pub fn generate_single_shot_receipt<D: AsRef<[u8]>>(
        &self,
        expiration_us: u64,
        extra_data: D,
    ) -> EyreResult<(Vec<u8>, EventualValueFuture<ReceiptEvent>)> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            bail!("network is not started");
        };

        let receipt_manager = self.receipt_manager();
        let routing_table = self.routing_table();

        // Generate receipt and serialized form to return
        let vcrypto = self.crypto().best();

        let nonce = vcrypto.random_nonce();
        let node_id = routing_table.node_id(vcrypto.kind());
        let node_id_secret = routing_table.node_id_secret_key(vcrypto.kind());

        let receipt = Receipt::try_new(
            best_envelope_version(),
            node_id.kind,
            nonce,
            node_id.value,
            extra_data,
        )?;
        let out = receipt
            .to_signed_data(self.crypto(), &node_id_secret)
            .wrap_err("failed to generate signed receipt")?;

        // Record the receipt for later
        let exp_ts = get_aligned_timestamp() + expiration_us;
        let eventual = SingleShotEventual::new(Some(ReceiptEvent::Cancelled));
        let instance = eventual.instance();
        receipt_manager.record_single_shot_receipt(receipt, exp_ts, eventual);

        Ok((out, instance))
    }

    /// Process a received out-of-band receipt
    #[instrument(level = "trace", target = "receipt", skip_all)]
    pub async fn handle_out_of_band_receipt<R: AsRef<[u8]>>(
        &self,
        receipt_data: R,
    ) -> NetworkResult<()> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return NetworkResult::service_unavailable("network is not started");
        };

        let receipt_manager = self.receipt_manager();

        let receipt = match Receipt::from_signed_data(self.crypto(), receipt_data.as_ref()) {
            Err(e) => {
                return NetworkResult::invalid_message(e.to_string());
            }
            Ok(v) => v,
        };

        receipt_manager
            .handle_receipt(receipt, ReceiptReturned::OutOfBand)
            .await
    }

    /// Process a received in-band receipt
    #[instrument(level = "trace", target = "receipt", skip_all)]
    pub async fn handle_in_band_receipt<R: AsRef<[u8]>>(
        &self,
        receipt_data: R,
        inbound_noderef: NodeRef,
    ) -> NetworkResult<()> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return NetworkResult::service_unavailable("network is not started");
        };

        let receipt_manager = self.receipt_manager();

        let receipt = match Receipt::from_signed_data(self.crypto(), receipt_data.as_ref()) {
            Err(e) => {
                return NetworkResult::invalid_message(e.to_string());
            }
            Ok(v) => v,
        };

        receipt_manager
            .handle_receipt(receipt, ReceiptReturned::InBand { inbound_noderef })
            .await
    }

    /// Process a received safety receipt
    #[instrument(level = "trace", target = "receipt", skip_all)]
    pub async fn handle_safety_receipt<R: AsRef<[u8]>>(
        &self,
        receipt_data: R,
    ) -> NetworkResult<()> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return NetworkResult::service_unavailable("network is not started");
        };

        let receipt_manager = self.receipt_manager();

        let receipt = match Receipt::from_signed_data(self.crypto(), receipt_data.as_ref()) {
            Err(e) => {
                return NetworkResult::invalid_message(e.to_string());
            }
            Ok(v) => v,
        };

        receipt_manager
            .handle_receipt(receipt, ReceiptReturned::Safety)
            .await
    }

    /// Process a received private receipt
    #[instrument(level = "trace", target = "receipt", skip_all)]
    pub async fn handle_private_receipt<R: AsRef<[u8]>>(
        &self,
        receipt_data: R,
        private_route: PublicKey,
    ) -> NetworkResult<()> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return NetworkResult::service_unavailable("network is not started");
        };

        let receipt_manager = self.receipt_manager();

        let receipt = match Receipt::from_signed_data(self.crypto(), receipt_data.as_ref()) {
            Err(e) => {
                return NetworkResult::invalid_message(e.to_string());
            }
            Ok(v) => v,
        };

        receipt_manager
            .handle_receipt(receipt, ReceiptReturned::Private { private_route })
            .await
    }

    // Process a received signal
    #[instrument(level = "trace", target = "net", skip_all)]
    pub async fn handle_signal(
        &self,
        signal_flow: Flow,
        signal_info: SignalInfo,
    ) -> EyreResult<NetworkResult<()>> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return Ok(NetworkResult::service_unavailable("network is not started"));
        };

        match signal_info {
            SignalInfo::ReverseConnect { receipt, peer_info } => {
                let routing_table = self.routing_table();
                let rpc = self.rpc_processor();

                // Add the peer info to our routing table
                let mut peer_nr = match routing_table.register_node_with_peer_info(
                    RoutingDomain::PublicInternet,
                    SafetyDomainSet::all(),
                    peer_info,
                    false,
                ) {
                    Ok(nr) => nr,
                    Err(e) => {
                        return Ok(NetworkResult::invalid_message(format!(
                            "unable to register reverse connect peerinfo: {}",
                            e
                        )));
                    }
                };

                // Restrict reverse connection to same sequencing requirement as inbound signal
                if signal_flow.protocol_type().is_ordered() {
                    peer_nr.set_sequencing(Sequencing::EnsureOrdered);
                }

                // Make a reverse connection to the peer and send the receipt to it
                rpc.rpc_call_return_receipt(Destination::direct(peer_nr), receipt)
                    .await
                    .wrap_err("rpc failure")
            }
            SignalInfo::HolePunch { receipt, peer_info } => {
                let routing_table = self.routing_table();
                let rpc = self.rpc_processor();

                // Add the peer info to our routing table
                let mut peer_nr = match routing_table.register_node_with_peer_info(
                    RoutingDomain::PublicInternet,
                    SafetyDomainSet::all(),
                    peer_info,
                    false,
                ) {
                    Ok(nr) => nr,
                    Err(e) => {
                        return Ok(NetworkResult::invalid_message(format!(
                            "unable to register hole punch connect peerinfo: {}",
                            e
                        )));
                    }
                };

                // Get the udp direct dialinfo for the hole punch
                let outbound_nrf = routing_table
                    .get_outbound_node_ref_filter(RoutingDomain::PublicInternet)
                    .with_protocol_type(ProtocolType::UDP);
                peer_nr.set_filter(Some(outbound_nrf));
                let Some(hole_punch_dial_info_detail) = peer_nr.first_filtered_dial_info_detail()
                else {
                    return Ok(NetworkResult::no_connection_other(format!(
                        "No hole punch capable dialinfo found for node: {}",
                        peer_nr
                    )));
                };

                // Now that we picked a specific dialinfo, further restrict the noderef to the specific address type
                let filter = peer_nr.take_filter().unwrap();
                let filter =
                    filter.with_address_type(hole_punch_dial_info_detail.dial_info.address_type());
                peer_nr.set_filter(Some(filter));

                // Do our half of the hole punch by sending an empty packet
                // Both sides will do this and then the receipt will get sent over the punched hole
                let unique_flow = network_result_try!(
                    self.net()
                        .send_data_to_dial_info(
                            hole_punch_dial_info_detail.dial_info.clone(),
                            Vec::new(),
                        )
                        .await?
                );

                // XXX: do we need a delay here? or another hole punch packet?

                // Set the hole punch as our 'last connection' to ensure we return the receipt over the direct hole punch
                peer_nr.set_last_flow(unique_flow.flow, get_aligned_timestamp());

                // Return the receipt using the same dial info send the receipt to it
                rpc.rpc_call_return_receipt(Destination::direct(peer_nr), receipt)
                    .await
                    .wrap_err("rpc failure")
            }
        }
    }

    /// Builds an envelope for sending over the network
    #[instrument(level = "trace", target = "net", skip_all)]
    fn build_envelope<B: AsRef<[u8]>>(
        &self,
        dest_node_id: TypedKey,
        version: u8,
        body: B,
    ) -> EyreResult<Vec<u8>> {
        // DH to get encryption key
        let routing_table = self.routing_table();
        let Some(vcrypto) = self.crypto().get(dest_node_id.kind) else {
            bail!("should not have a destination with incompatible crypto here");
        };

        let node_id = routing_table.node_id(vcrypto.kind());
        let node_id_secret = routing_table.node_id_secret_key(vcrypto.kind());

        // Get timestamp, nonce
        let ts = get_aligned_timestamp();
        let nonce = vcrypto.random_nonce();

        // Encode envelope
        let envelope = Envelope::new(
            version,
            node_id.kind,
            ts,
            nonce,
            node_id.value,
            dest_node_id.value,
        );
        envelope
            .to_encrypted_data(
                self.crypto(),
                body.as_ref(),
                &node_id_secret,
                &self.unlocked_inner.network_key,
            )
            .wrap_err("envelope failed to encode")
    }

    /// Called by the RPC handler when we want to issue an RPC request or response
    /// safety_domain is used to determine if this is being sent in an unsafe context
    /// and should reject attempts to send to safety-only nodes
    /// node_ref is the direct destination to which the envelope will be sent
    /// If 'destination_node_ref' is specified, it can be different than the node_ref being sent to
    /// which will cause the envelope to be relayed
    #[instrument(level = "trace", target = "net", skip_all)]
    pub async fn send_envelope<B: AsRef<[u8]>>(
        &self,
        safety_domain: SafetyDomain,
        node_ref: NodeRef,
        destination_node_ref: Option<NodeRef>,
        body: B,
    ) -> EyreResult<NetworkResult<SendDataMethod>> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return Ok(NetworkResult::no_connection_other("network is not started"));
        };

        let destination_node_ref = destination_node_ref.as_ref().unwrap_or(&node_ref).clone();
        let best_node_id = destination_node_ref.best_node_id();

        // Get node's envelope versions and see if we can send to it
        // and if so, get the max version we can use
        let Some(envelope_version) = destination_node_ref.best_envelope_version() else {
            bail!(
                "can't talk to this node {} because we dont support its envelope versions",
                node_ref
            );
        };

        // Build the envelope to send
        let out = self.build_envelope(best_node_id, envelope_version, body)?;

        if !node_ref.same_entry(&destination_node_ref) {
            log_net!(
                "sending envelope to {:?} via {:?}, len={}",
                destination_node_ref,
                node_ref,
                out.len()
            );
        } else {
            log_net!("sending envelope to {:?}, len={}", node_ref, out.len());
        }

        // Send the envelope via whatever means necessary
        self.send_data(safety_domain, node_ref, out).await
    }

    /// Called by the RPC handler when we want to issue an direct receipt
    #[instrument(level = "trace", target = "receipt", skip_all)]
    pub async fn send_out_of_band_receipt(
        &self,
        dial_info: DialInfo,
        rcpt_data: Vec<u8>,
    ) -> EyreResult<()> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            log_net!(debug "not sending out-of-band receipt to {} because network is stopped", dial_info);
            return Ok(());
        };

        // Do we need to validate the outgoing receipt? Probably not
        // because it is supposed to be opaque and the
        // recipient/originator does the validation
        // Also, in the case of an old 'version', returning the receipt
        // should not be subject to our ability to decode it

        // Send receipt directly
        network_result_value_or_log!(self
            .net()
            .send_data_unbound_to_dial_info(dial_info, rcpt_data)
            .await? => [ format!(": dial_info={}, rcpt_data.len={}", dial_info, rcpt_data.len()) ] {
                return Ok(());
            }
        );
        Ok(())
    }

    // Called when a packet potentially containing an RPC envelope is received by a low-level
    // network protocol handler. Processes the envelope, authenticates and decrypts the RPC message
    // and passes it to the RPC handler
    #[instrument(level = "trace", target = "net", skip_all)]
    async fn on_recv_envelope(&self, data: &mut [u8], flow: Flow) -> EyreResult<bool> {
        let Ok(_guard) = self.unlocked_inner.startup_lock.enter() else {
            return Ok(false);
        };

        log_net!("envelope of {} bytes received from {:?}", data.len(), flow);
        let remote_addr = flow.remote_address().ip_addr();

        // Network accounting
        self.stats_packet_rcvd(remote_addr, ByteCount::new(data.len() as u64));

        // If this is a zero length packet, just drop it, because these are used for hole punching
        // and possibly other low-level network connectivity tasks and will never require
        // more processing or forwarding
        if data.is_empty() {
            return Ok(true);
        }

        // Ensure we can read the magic number
        if data.len() < 4 {
            log_net!(debug "short packet");
            self.address_filter()
                .punish_ip_addr(remote_addr, PunishmentReason::ShortPacket);
            return Ok(false);
        }

        // Get the routing domain for this data
        let routing_domain = match self
            .routing_table()
            .routing_domain_for_address(flow.remote_address().address())
        {
            Some(rd) => rd,
            None => {
                log_net!(debug "no routing domain for envelope received from {:?}", flow);
                return Ok(false);
            }
        };

        // Is this a direct bootstrap request instead of an envelope?
        if data[0..4] == *BOOT_MAGIC {
            network_result_value_or_log!(self.handle_boot_request(flow).await? => [ format!(": flow={:?}", flow) ] {});
            return Ok(true);
        }

        // Is this an out-of-band receipt instead of an envelope?
        if data[0..3] == *RECEIPT_MAGIC {
            network_result_value_or_log!(self.handle_out_of_band_receipt(data).await => [ format!(": data.len={}", data.len()) ] {});
            return Ok(true);
        }

        // Decode envelope header (may fail signature validation)
        let envelope =
            match Envelope::from_signed_data(self.crypto(), data, &self.unlocked_inner.network_key)
            {
                Ok(v) => v,
                Err(e) => {
                    log_net!(debug "envelope failed to decode: {}", e);
                    // safe to punish here because relays also check here to ensure they arent forwarding things that don't decode
                    self.address_filter()
                        .punish_ip_addr(remote_addr, PunishmentReason::FailedToDecodeEnvelope);
                    return Ok(false);
                }
            };

        // Get timestamp range
        let (tsbehind, tsahead) = self.with_config(|c| {
            (
                c.network
                    .rpc
                    .max_timestamp_behind_ms
                    .map(ms_to_us)
                    .map(TimestampDuration::new),
                c.network
                    .rpc
                    .max_timestamp_ahead_ms
                    .map(ms_to_us)
                    .map(TimestampDuration::new),
            )
        });

        // Validate timestamp isn't too old
        let ts = get_aligned_timestamp();
        let ets = envelope.get_timestamp();
        if let Some(tsbehind) = tsbehind {
            if tsbehind.as_u64() != 0 && (ts > ets && ts.saturating_sub(ets) > tsbehind) {
                log_net!(debug
                    "Timestamp behind: {}ms ({})",
                    timestamp_to_secs(ts.saturating_sub(ets).as_u64()) * 1000f64,
                    flow.remote()
                );
                return Ok(false);
            }
        }
        if let Some(tsahead) = tsahead {
            if tsahead.as_u64() != 0 && (ts < ets && ets.saturating_sub(ts) > tsahead) {
                log_net!(debug
                    "Timestamp ahead: {}ms ({})",
                    timestamp_to_secs(ets.saturating_sub(ts).as_u64()) * 1000f64,
                    flow.remote()
                );
                return Ok(false);
            }
        }

        // Get routing table and rpc processor
        let routing_table = self.routing_table();
        let rpc = self.rpc_processor();

        // Peek at header and see if we need to relay this
        // If the recipient id is not our node id, then it needs relaying
        let sender_id = envelope.get_sender_typed_id();
        if self.address_filter().is_node_id_punished(sender_id) {
            return Ok(false);
        }

        let recipient_id = envelope.get_recipient_typed_id();
        if !routing_table.matches_own_node_id(&[recipient_id]) {
            // See if the source node is allowed to resolve nodes
            // This is a costly operation, so only outbound-relay permitted
            // nodes are allowed to do this, for example PWA users

            let some_relay_nr = if self.check_client_allowlist(sender_id) {
                // Full relay allowed, do a full resolve_node
                match rpc
                    .resolve_node(recipient_id, SafetySelection::Unsafe(Sequencing::default()))
                    .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        log_net!(debug "failed to resolve recipient node for relay, dropping outbound relayed packet: {}" ,e);
                        return Ok(false);
                    }
                }
            } else {
                // If this is not a node in the client allowlist, only allow inbound relay
                // which only performs a lightweight lookup before passing the packet back out

                // See if we have the node in our routing table
                // We should, because relays are chosen by nodes that have established connectivity and
                // should be mutually in each others routing tables. The node needing the relay will be
                // pinging this node regularly to keep itself in the routing table
                match routing_table.lookup_node_ref(recipient_id) {
                    Ok(v) => v,
                    Err(e) => {
                        log_net!(debug "failed to look up recipient node for relay, dropping outbound relayed packet: {}" ,e);
                        return Ok(false);
                    }
                }
            };

            if let Some(mut relay_nr) = some_relay_nr {
                // Ensure the protocol used to forward is of the same sequencing requirement
                // Address type is allowed to change if connectivity is better
                if flow.protocol_type().is_ordered() {
                    relay_nr.set_sequencing(Sequencing::EnsureOrdered);
                };

                // Relay the packet to the desired destination
                // Relayed packets are never received over a safety route so they are implicitly
                // in the SafetyDomain::Unsafe
                log_net!("relaying {} bytes to {}", data.len(), relay_nr);
                network_result_value_or_log!(match self.send_data(SafetyDomain::Unsafe, relay_nr, data.to_vec())
                    .await {
                        Ok(v) => v,
                        Err(e) => {
                            log_net!(debug "failed to forward envelope: {}" ,e);
                            return Ok(false);
                        }
                    } => [ format!(": relay_nr={}, data.len={}", relay_nr, data.len()) ] {
                        return Ok(false);
                    }
                );
            }
            // Inform caller that we dealt with the envelope, but did not process it locally
            return Ok(false);
        }

        // DH to get decryption key (cached)
        let node_id_secret = routing_table.node_id_secret_key(envelope.get_crypto_kind());

        // Decrypt the envelope body
        let body = match envelope.decrypt_body(
            self.crypto(),
            data,
            &node_id_secret,
            &self.unlocked_inner.network_key,
        ) {
            Ok(v) => v,
            Err(e) => {
                log_net!(debug "failed to decrypt envelope body: {}", e);
                // Can't punish by ip address here because relaying can't decrypt envelope bodies to check
                // But because the envelope was properly signed by the time it gets here, it is safe to
                // punish by node id
                self.address_filter()
                    .punish_node_id(sender_id, PunishmentReason::FailedToDecryptEnvelopeBody);
                return Ok(false);
            }
        };

        // Cache the envelope information in the routing table
        let mut source_noderef = match routing_table.register_node_with_existing_connection(
            envelope.get_sender_typed_id(),
            flow,
            ts,
        ) {
            Ok(v) => v,
            Err(e) => {
                // If the node couldn't be registered just skip this envelope,
                log_net!(debug "failed to register node with existing connection: {}", e);
                return Ok(false);
            }
        };
        source_noderef.add_envelope_version(envelope.get_version());

        // Enforce routing domain
        source_noderef.merge_filter(NodeRefFilter::new().with_routing_domain(routing_domain));

        // Pass message to RPC system
        if let Err(e) =
            rpc.enqueue_direct_message(envelope, source_noderef, flow, routing_domain, body)
        {
            // Couldn't enqueue, but not the sender's fault
            log_net!(debug "failed to enqueue direct message: {}", e);
            return Ok(false);
        }

        // Inform caller that we dealt with the envelope locally
        Ok(true)
    }

    pub fn restart_network(&self) {
        self.net().restart_network();
    }
}
