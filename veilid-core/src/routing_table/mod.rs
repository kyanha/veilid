mod bucket;
mod bucket_entry;
mod debug;
mod find_nodes;
mod node_ref;
mod route_spec_store;
mod routing_domain_editor;
mod routing_domains;
mod stats_accounting;
mod tasks;

use crate::dht::*;
use crate::network_manager::*;
use crate::rpc_processor::*;
use crate::xx::*;
use crate::*;
use bucket::*;
pub use bucket_entry::*;
pub use debug::*;
pub use find_nodes::*;
use hashlink::LruCache;
pub use node_ref::*;
pub use route_spec_store::*;
pub use routing_domain_editor::*;
pub use routing_domains::*;
pub use stats_accounting::*;

const RECENT_PEERS_TABLE_SIZE: usize = 64;

//////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub struct RecentPeersEntry {
    pub last_connection: ConnectionDescriptor,
}

/// RoutingTable rwlock-internal data
struct RoutingTableInner {
    /// Routing table buckets that hold entries
    buckets: Vec<Bucket>,
    /// A fast counter for the number of entries in the table, total
    bucket_entry_count: usize,
    /// The public internet routing domain
    public_internet_routing_domain: PublicInternetRoutingDomainDetail,
    /// The dial info we use on the local network
    local_network_routing_domain: LocalNetworkRoutingDomainDetail,
    /// Interim accounting mechanism for this node's RPC latency to any other node
    self_latency_stats_accounting: LatencyStatsAccounting,
    /// Interim accounting mechanism for the total bandwidth to/from this node
    self_transfer_stats_accounting: TransferStatsAccounting,
    /// Statistics about the total bandwidth to/from this node
    self_transfer_stats: TransferStatsDownUp,
    /// Peers we have recently communicated with
    recent_peers: LruCache<DHTKey, RecentPeersEntry>,
    /// Storage for private/safety RouteSpecs
    route_spec_store: RouteSpecStore,
}

#[derive(Clone, Debug, Default)]
pub struct RoutingTableHealth {
    /// Number of reliable (responsive) entries in the routing table
    pub reliable_entry_count: usize,
    /// Number of unreliable (occasionally unresponsive) entries in the routing table
    pub unreliable_entry_count: usize,
    /// Number of dead (always unresponsive) entries in the routing table
    pub dead_entry_count: usize,
}

struct RoutingTableUnlockedInner {
    // Accessors
    config: VeilidConfig,
    network_manager: NetworkManager,

    /// The current node's public DHT key
    node_id: DHTKey,
    /// The current node's DHT key secret
    node_id_secret: DHTKeySecret,
    /// Buckets to kick on our next kick task
    kick_queue: Mutex<BTreeSet<usize>>,
    /// Background process for computing statistics
    rolling_transfers_task: TickTask<EyreReport>,
    /// Backgroup process to purge dead routing table entries when necessary
    kick_buckets_task: TickTask<EyreReport>,
}

#[derive(Clone)]
pub struct RoutingTable {
    inner: Arc<RwLock<RoutingTableInner>>,
    unlocked_inner: Arc<RoutingTableUnlockedInner>,
}

impl RoutingTable {
    fn new_inner(config: VeilidConfig) -> RoutingTableInner {
        RoutingTableInner {
            buckets: Vec::new(),
            public_internet_routing_domain: PublicInternetRoutingDomainDetail::default(),
            local_network_routing_domain: LocalNetworkRoutingDomainDetail::default(),
            bucket_entry_count: 0,
            self_latency_stats_accounting: LatencyStatsAccounting::new(),
            self_transfer_stats_accounting: TransferStatsAccounting::new(),
            self_transfer_stats: TransferStatsDownUp::default(),
            recent_peers: LruCache::new(RECENT_PEERS_TABLE_SIZE),
            route_spec_store: RouteSpecStore::new(config),
        }
    }
    fn new_unlocked_inner(
        config: VeilidConfig,
        network_manager: NetworkManager,
    ) -> RoutingTableUnlockedInner {
        let c = config.get();
        RoutingTableUnlockedInner {
            config: config.clone(),
            network_manager,
            node_id: c.network.node_id,
            node_id_secret: c.network.node_id_secret,
            kick_queue: Mutex::new(BTreeSet::default()),
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
            kick_buckets_task: TickTask::new(1),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        let config = network_manager.config();
        let this = Self {
            inner: Arc::new(RwLock::new(Self::new_inner(config.clone()))),
            unlocked_inner: Arc::new(Self::new_unlocked_inner(config, network_manager)),
        };
        // Set rolling transfers tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .rolling_transfers_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this2
                            .clone()
                            .rolling_transfers_task_routine(s, l, t)
                            .instrument(trace_span!(
                                parent: None,
                                "RoutingTable rolling transfers task routine"
                            )),
                    )
                });
        }

        // Set kick buckets tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .kick_buckets_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this2
                            .clone()
                            .kick_buckets_task_routine(s, l, t)
                            .instrument(trace_span!(parent: None, "kick buckets task routine")),
                    )
                });
        }
        this
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.unlocked_inner.network_manager.clone()
    }
    pub fn rpc_processor(&self) -> RPCProcessor {
        self.network_manager().rpc_processor()
    }

    pub fn node_id(&self) -> DHTKey {
        self.unlocked_inner.node_id
    }

    pub fn node_id_secret(&self) -> DHTKeySecret {
        self.unlocked_inner.node_id_secret
    }

    fn routing_domain_for_address_inner(
        inner: &RoutingTableInner,
        address: Address,
    ) -> Option<RoutingDomain> {
        for rd in RoutingDomain::all() {
            let can_contain =
                Self::with_routing_domain(inner, rd, |rdd| rdd.can_contain_address(address));
            if can_contain {
                return Some(rd);
            }
        }
        None
    }

    pub fn routing_domain_for_address(&self, address: Address) -> Option<RoutingDomain> {
        let inner = self.inner.read();
        Self::routing_domain_for_address_inner(&*inner, address)
    }

    fn with_routing_domain<F, R>(inner: &RoutingTableInner, domain: RoutingDomain, f: F) -> R
    where
        F: FnOnce(&dyn RoutingDomainDetail) -> R,
    {
        match domain {
            RoutingDomain::PublicInternet => f(&inner.public_internet_routing_domain),
            RoutingDomain::LocalNetwork => f(&inner.local_network_routing_domain),
        }
    }

    fn with_routing_domain_mut<F, R>(
        inner: &mut RoutingTableInner,
        domain: RoutingDomain,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut dyn RoutingDomainDetail) -> R,
    {
        match domain {
            RoutingDomain::PublicInternet => f(&mut inner.public_internet_routing_domain),
            RoutingDomain::LocalNetwork => f(&mut inner.local_network_routing_domain),
        }
    }

    pub fn with_route_spec_store_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut RouteSpecStore, &mut RoutingTableInner) -> R,
    {
        let inner = &mut *self.inner.write();
        f(&mut inner.route_spec_store, inner)
    }

    pub fn with_route_spec_store<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&RouteSpecStore, &RoutingTableInner) -> R,
    {
        let inner = &*self.inner.read();
        f(&inner.route_spec_store, inner)
    }

    pub fn relay_node(&self, domain: RoutingDomain) -> Option<NodeRef> {
        let inner = self.inner.read();
        Self::with_routing_domain(&*inner, domain, |rd| rd.common().relay_node())
    }

    pub fn has_dial_info(&self, domain: RoutingDomain) -> bool {
        let inner = self.inner.read();
        Self::with_routing_domain(&*inner, domain, |rd| {
            !rd.common().dial_info_details().is_empty()
        })
    }

    pub fn dial_info_details(&self, domain: RoutingDomain) -> Vec<DialInfoDetail> {
        let inner = self.inner.read();
        Self::with_routing_domain(&*inner, domain, |rd| {
            rd.common().dial_info_details().clone()
        })
    }

    pub fn first_filtered_dial_info_detail(
        &self,
        routing_domain_set: RoutingDomainSet,
        filter: &DialInfoFilter,
    ) -> Option<DialInfoDetail> {
        let inner = self.inner.read();
        for routing_domain in routing_domain_set {
            let did = Self::with_routing_domain(&*inner, routing_domain, |rd| {
                for did in rd.common().dial_info_details() {
                    if did.matches_filter(filter) {
                        return Some(did.clone());
                    }
                }
                None
            });
            if did.is_some() {
                return did;
            }
        }
        None
    }

    pub fn all_filtered_dial_info_details(
        &self,
        routing_domain_set: RoutingDomainSet,
        filter: &DialInfoFilter,
    ) -> Vec<DialInfoDetail> {
        let inner = self.inner.read();
        let mut ret = Vec::new();
        for routing_domain in routing_domain_set {
            Self::with_routing_domain(&*inner, routing_domain, |rd| {
                for did in rd.common().dial_info_details() {
                    if did.matches_filter(filter) {
                        ret.push(did.clone());
                    }
                }
            });
        }
        ret.remove_duplicates();
        ret
    }

    pub fn ensure_dial_info_is_valid(&self, domain: RoutingDomain, dial_info: &DialInfo) -> bool {
        let address = dial_info.socket_address().address();
        let inner = self.inner.read();
        let can_contain_address =
            Self::with_routing_domain(&*inner, domain, |rd| rd.can_contain_address(address));

        if !can_contain_address {
            log_rtab!(debug "can not add dial info to this routing domain");
            return false;
        }
        if !dial_info.is_valid() {
            log_rtab!(debug
                "shouldn't be registering invalid addresses: {:?}",
                dial_info
            );
            return false;
        }
        true
    }

    pub fn node_info_is_valid_in_routing_domain(
        &self,
        routing_domain: RoutingDomain,
        node_info: &NodeInfo,
    ) -> bool {
        // Should not be passing around nodeinfo with an invalid network class
        if matches!(node_info.network_class, NetworkClass::Invalid) {
            return false;
        }
        // Ensure all of the dial info works in this routing domain
        for did in &node_info.dial_info_detail_list {
            if !self.ensure_dial_info_is_valid(routing_domain, &did.dial_info) {
                return false;
            }
        }
        // Ensure the relay is also valid in this routing domain if it is provided
        if let Some(relay_peer_info) = node_info.relay_peer_info.as_ref() {
            let relay_ni = &relay_peer_info.signed_node_info.node_info;
            if !self.node_info_is_valid_in_routing_domain(routing_domain, relay_ni) {
                return false;
            }
        }
        true
    }

    #[instrument(level = "trace", skip(inner), ret)]
    fn get_contact_method_inner(
        inner: &RoutingTableInner,
        routing_domain: RoutingDomain,
        node_a_id: &DHTKey,
        node_a: &NodeInfo,
        node_b_id: &DHTKey,
        node_b: &NodeInfo,
        dial_info_filter: DialInfoFilter,
        reliable: bool,
    ) -> ContactMethod {
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.get_contact_method(
                inner,
                node_a_id,
                node_a,
                node_b_id,
                node_b,
                dial_info_filter,
                reliable,
            )
        })
    }

    /// Look up the best way for two nodes to reach each other over a specific routing domain
    #[instrument(level = "trace", skip(self), ret)]
    pub fn get_contact_method(
        &self,
        routing_domain: RoutingDomain,
        node_a_id: &DHTKey,
        node_a: &NodeInfo,
        node_b_id: &DHTKey,
        node_b: &NodeInfo,
        dial_info_filter: DialInfoFilter,
        reliable: bool,
    ) -> ContactMethod {
        let inner = &*self.inner.read();
        Self::get_contact_method_inner(
            inner,
            routing_domain,
            node_a_id,
            node_a,
            node_b_id,
            node_b,
            dial_info_filter,
            reliable,
        )
    }

    // Figure out how to reach a node from our own node over the best routing domain and reference the nodes we want to access
    #[instrument(level = "trace", skip(self), ret)]
    pub(crate) fn get_node_contact_method(
        &self,
        target_node_ref: NodeRef,
    ) -> EyreResult<NodeContactMethod> {
        // Lock the routing table for read to ensure the table doesn't change
        let inner = &*self.inner.read();

        // Figure out the best routing domain to get the contact method over
        let routing_domain = match target_node_ref.best_routing_domain() {
            Some(rd) => rd,
            None => {
                log_net!("no routing domain for node {:?}", target_node_ref);
                return Ok(NodeContactMethod::Unreachable);
            }
        };

        // Node A is our own node
        let node_a = get_own_node_info_inner(inner, routing_domain);
        let node_a_id = self.node_id();

        // Node B is the target node
        let node_b = target_node_ref.xxx operate(|_rti, e| e.node_info(routing_domain).unwrap());
        let node_b_id = target_node_ref.node_id();

        // Dial info filter comes from the target node ref
        let dial_info_filter = target_node_ref.dial_info_filter();
        let reliable = target_node_ref.reliable();

        let cm = self.get_contact_method(
            routing_domain,
            &node_a_id,
            &node_a,
            &node_b_id,
            node_b,
            dial_info_filter,
            reliable,
        );

        // Translate the raw contact method to a referenced contact method
        Ok(match cm {
            ContactMethod::Unreachable => NodeContactMethod::Unreachable,
            ContactMethod::Existing => NodeContactMethod::Existing,
            ContactMethod::Direct(di) => NodeContactMethod::Direct(di),
            ContactMethod::SignalReverse(relay_key, target_key) => {
                let relay_nr = Self::lookup_and_filter_noderef_inner(inner, self.clone(), relay_key, routing_domain.into(), dial_info_filter)
                    .ok_or_else(|| eyre!("couldn't look up relay"))?;
                if target_node_ref.node_id() != target_key {
                    bail!("target noderef didn't match target key");
                }
                NodeContactMethod::SignalReverse(relay_nr, target_node_ref)
            }
            ContactMethod::SignalHolePunch(relay_key, target_key) => {
                let relay_nr = Self::lookup_and_filter_noderef_inner(inner, self.clone(), relay_key, routing_domain.into(), dial_info_filter)
                    .ok_or_else(|| eyre!("couldn't look up relay"))?;
                if target_node_ref.node_id() != target_key {
                    bail!("target noderef didn't match target key");
                }
                NodeContactMethod::SignalHolePunch(relay_nr, target_node_ref)
            }
            ContactMethod::InboundRelay(relay_key) => {
                let relay_nr = Self::lookup_and_filter_noderef_nner(inner, self.clone(), relay_key, routing_domain.into(), dial_info_filter)
                    .ok_or_else(|| eyre!("couldn't look up relay"))?;
                NodeContactMethod::InboundRelay(relay_nr)
            }
            ContactMethod::OutboundRelay(relay_key) => {
                let relay_nr = Self::lookup_and_filter_noderef(inner, self.clone(), relay_key, routing_domain.into(), dial_info_filter)
                    .ok_or_else(|| eyre!("couldn't look up relay"))?;
                NodeContactMethod::OutboundRelay(relay_nr)
            }
        })
    }

    #[instrument(level = "debug", skip(self))]
    pub fn edit_routing_domain(&self, domain: RoutingDomain) -> RoutingDomainEditor {
        RoutingDomainEditor::new(self.clone(), domain)
    }

    fn reset_all_seen_our_node_info(inner: &mut RoutingTableInner, routing_domain: RoutingDomain) {
        let cur_ts = intf::get_timestamp();
        Self::with_entries_mut(inner, cur_ts, BucketEntryState::Dead, |rti, _, v| {
            v.with_mut(rti, |_rti, e| {
                e.set_seen_our_node_info(routing_domain, false);
            });
            Option::<()>::None
        });
    }

    fn reset_all_updated_since_last_network_change(inner: &mut RoutingTableInner) {
        let cur_ts = intf::get_timestamp();
        Self::with_entries_mut(inner, cur_ts, BucketEntryState::Dead, |rti, _, v| {
            v.with_mut(rti, |_rti, e| {
                e.set_updated_since_last_network_change(false)
            });
            Option::<()>::None
        });
    }

    /// Return a copy of our node's peerinfo
    pub fn get_own_peer_info(&self, routing_domain: RoutingDomain) -> PeerInfo {
        let inner = &*self.inner.read();
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.common().with_peer_info(|pi| pi.clone())
        })
    }

    /// Return a copy of our node's signednodeinfo
    pub fn get_own_signed_node_info(&self, routing_domain: RoutingDomain) -> SignedNodeInfo {
        let inner = &*self.inner.read();
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.common()
                .with_peer_info(|pi| pi.signed_node_info.clone())
        })
    }

    /// Return a copy of our node's nodeinfo
    fn get_own_node_info_inner(
        inner: &RoutingTableInner,
        routing_domain: RoutingDomain,
    ) -> NodeInfo {
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.common()
                .with_peer_info(|pi| pi.signed_node_info.node_info.clone())
        })
    }
    pub fn get_own_node_info(&self, routing_domain: RoutingDomain) -> NodeInfo {
        let inner = &*self.inner.read();
        Self::get_own_node_info_inner(inner, routing_domain)
    }

    /// Return our currently registered network class
    pub fn has_valid_own_node_info(&self, routing_domain: RoutingDomain) -> bool {
        let inner = &*self.inner.read();
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.common().has_valid_own_node_info()
        })
    }

    /// Return the domain's currently registered network class
    pub fn get_network_class(&self, routing_domain: RoutingDomain) -> Option<NetworkClass> {
        let inner = &*self.inner.read();
        Self::with_routing_domain(inner, routing_domain, |rdd| rdd.common().network_class())
    }

    /// Return the domain's filter for what we can receivein the form of a dial info filter
    pub fn get_inbound_dial_info_filter(&self, routing_domain: RoutingDomain) -> DialInfoFilter {
        let inner = &*self.inner.read();
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.common().inbound_dial_info_filter()
        })
    }

    /// Return the domain's filter for what we can receive in the form of a node ref filter
    pub fn get_inbound_node_ref_filter(&self, routing_domain: RoutingDomain) -> NodeRefFilter {
        let dif = self.get_inbound_dial_info_filter(routing_domain);
        NodeRefFilter::new()
            .with_routing_domain(routing_domain)
            .with_dial_info_filter(dif)
    }

    /// Return the domain's filter for what we can send out in the form of a dial info filter
    pub fn get_outbound_dial_info_filter(&self, routing_domain: RoutingDomain) -> DialInfoFilter {
        let inner = &*self.inner.read();
        Self::with_routing_domain(inner, routing_domain, |rdd| {
            rdd.common().outbound_dial_info_filter()
        })
    }
    /// Return the domain's filter for what we can receive in the form of a node ref filter
    pub fn get_outbound_node_ref_filter(&self, routing_domain: RoutingDomain) -> NodeRefFilter {
        let dif = self.get_outbound_dial_info_filter(routing_domain);
        NodeRefFilter::new()
            .with_routing_domain(routing_domain)
            .with_dial_info_filter(dif)
    }

    fn bucket_depth(index: usize) -> usize {
        match index {
            0 => 256,
            1 => 128,
            2 => 64,
            3 => 32,
            4 => 16,
            5 => 8,
            6 => 4,
            7 => 4,
            8 => 4,
            9 => 4,
            _ => 4,
        }
    }

    pub async fn init(&self) -> EyreResult<()> {
        let mut inner = self.inner.write();
        // Size the buckets (one per bit)
        inner.buckets.reserve(DHT_KEY_LENGTH * 8);
        for _ in 0..DHT_KEY_LENGTH * 8 {
            let bucket = Bucket::new(self.clone());
            inner.buckets.push(bucket);
        }

        Ok(())
    }

    pub async fn terminate(&self) {
        debug!("starting routing table terminate");

        // Cancel all tasks being ticked
        debug!("stopping rolling transfers task");
        if let Err(e) = self.unlocked_inner.rolling_transfers_task.stop().await {
            error!("rolling_transfers_task not stopped: {}", e);
        }
        debug!("stopping kick buckets task");
        if let Err(e) = self.unlocked_inner.kick_buckets_task.stop().await {
            error!("kick_buckets_task not stopped: {}", e);
        }

        *self.inner.write() = Self::new_inner(self.unlocked_inner.config.clone());

        debug!("finished routing table terminate");
    }

    pub fn configure_local_network_routing_domain(&self, local_networks: Vec<(IpAddr, IpAddr)>) {
        log_net!(debug "configure_local_network_routing_domain: {:#?}", local_networks);

        let mut inner = self.inner.write();
        let changed = inner
            .local_network_routing_domain
            .set_local_networks(local_networks);

        // If the local network topology has changed, nuke the existing local node info and let new local discovery happen
        if changed {
            let cur_ts = intf::get_timestamp();
            Self::with_entries_mut(&mut *inner, cur_ts, BucketEntryState::Dead, |rti, _, e| {
                e.with_mut(rti, |_rti, e| {
                    e.clear_signed_node_info(RoutingDomain::LocalNetwork);
                    e.set_seen_our_node_info(RoutingDomain::LocalNetwork, false);
                    e.set_updated_since_last_network_change(false);
                });
                Option::<()>::None
            });
        }
    }

    /// Attempt to empty the routing table
    /// should only be performed when there are no node_refs (detached)
    pub fn purge_buckets(&self) {
        let mut inner = self.inner.write();
        let inner = &mut *inner;
        log_rtab!(
            "Starting routing table buckets purge. Table currently has {} nodes",
            inner.bucket_entry_count
        );
        for bucket in &inner.buckets {
            bucket.kick(inner, 0);
        }
        log_rtab!(debug
             "Routing table buckets purge complete. Routing table now has {} nodes",
            inner.bucket_entry_count
        );
    }

    /// Attempt to remove last_connections from entries
    pub fn purge_last_connections(&self) {
        let mut inner = self.inner.write();
        let inner = &mut *inner;
        log_rtab!(
            "Starting routing table last_connections purge. Table currently has {} nodes",
            inner.bucket_entry_count
        );
        for bucket in &inner.buckets {
            for entry in bucket.entries() {
                entry.1.with_mut(inner, |_rti, e| {
                    e.clear_last_connections();
                });
            }
        }
        log_rtab!(debug
             "Routing table last_connections purge complete. Routing table now has {} nodes",
            inner.bucket_entry_count
        );
    }

    /// Attempt to settle buckets and remove entries down to the desired number
    /// which may not be possible due extant NodeRefs
    fn kick_bucket(inner: &mut RoutingTableInner, idx: usize) {
        let bucket = &mut inner.buckets[idx];
        let bucket_depth = Self::bucket_depth(idx);

        if let Some(dead_node_ids) = bucket.kick(inner, bucket_depth) {
            // Remove counts
            inner.bucket_entry_count -= dead_node_ids.len();
            log_rtab!(debug "Routing table now has {} nodes", inner.bucket_entry_count);

            // Now purge the routing table inner vectors
            //let filter = |k: &DHTKey| dead_node_ids.contains(k);
            //inner.closest_reliable_nodes.retain(filter);
            //inner.fastest_reliable_nodes.retain(filter);
            //inner.closest_nodes.retain(filter);
            //inner.fastest_nodes.retain(filter);
        }
    }

    fn find_bucket_index(&self, node_id: DHTKey) -> usize {
        distance(&node_id, &self.unlocked_inner.node_id)
            .first_nonzero_bit()
            .unwrap()
    }

    pub fn get_entry_count(
        &self,
        routing_domain_set: RoutingDomainSet,
        min_state: BucketEntryState,
    ) -> usize {
        let inner = self.inner.read();
        Self::get_entry_count_inner(&*inner, routing_domain_set, min_state)
    }

    fn get_entry_count_inner(
        inner: &RoutingTableInner,
        routing_domain_set: RoutingDomainSet,
        min_state: BucketEntryState,
    ) -> usize {
        let mut count = 0usize;
        let cur_ts = intf::get_timestamp();
        Self::with_entries(inner, cur_ts, min_state, |rti, _, e| {
            if e.with(rti, |_rti, e| e.best_routing_domain(routing_domain_set))
                .is_some()
            {
                count += 1;
            }
            Option::<()>::None
        });
        count
    }

    fn with_entries<T, F: FnMut(&RoutingTableInner, DHTKey, Arc<BucketEntry>) -> Option<T>>(
        inner: &RoutingTableInner,
        cur_ts: u64,
        min_state: BucketEntryState,
        mut f: F,
    ) -> Option<T> {
        for bucket in &inner.buckets {
            for entry in bucket.entries() {
                if entry.1.with(inner, |_rti, e| e.state(cur_ts) >= min_state) {
                    if let Some(out) = f(inner, *entry.0, entry.1.clone()) {
                        return Some(out);
                    }
                }
            }
        }
        None
    }

    fn with_entries_mut<
        T,
        F: FnMut(&mut RoutingTableInner, DHTKey, Arc<BucketEntry>) -> Option<T>,
    >(
        inner: &mut RoutingTableInner,
        cur_ts: u64,
        min_state: BucketEntryState,
        mut f: F,
    ) -> Option<T> {
        for bucket in &inner.buckets {
            for entry in bucket.entries() {
                if entry.1.with(inner, |_rti, e| e.state(cur_ts) >= min_state) {
                    if let Some(out) = f(inner, *entry.0, entry.1.clone()) {
                        return Some(out);
                    }
                }
            }
        }
        None
    }

    pub fn get_nodes_needing_updates(
        &self,
        routing_domain: RoutingDomain,
        cur_ts: u64,
        all: bool,
    ) -> Vec<NodeRef> {
        let inner = self.inner.read();
        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
        Self::with_entries(
            &*inner,
            cur_ts,
            BucketEntryState::Unreliable,
            |rti, k, v| {
                // Only update nodes that haven't seen our node info yet
                if all || !v.with(rti, |_rti, e| e.has_seen_our_node_info(routing_domain)) {
                    node_refs.push(NodeRef::new(
                        self.clone(),
                        k,
                        v,
                        Some(NodeRefFilter::new().with_routing_domain(routing_domain)),
                    ));
                }
                Option::<()>::None
            },
        );
        node_refs
    }

    pub fn get_nodes_needing_ping(
        &self,
        routing_domain: RoutingDomain,
        cur_ts: u64,
    ) -> Vec<NodeRef> {
        let inner = self.inner.read();

        // Collect relay nodes
        let opt_relay_id = Self::with_routing_domain(&*inner, routing_domain, |rd| {
            rd.common().relay_node().map(|rn| rn.node_id())
        });

        // Collect all entries that are 'needs_ping' and have some node info making them reachable somehow
        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
        Self::with_entries(
            &*inner,
            cur_ts,
            BucketEntryState::Unreliable,
            |rti, k, v| {
                if v.with(rti, |_rti, e| {
                    e.has_node_info(routing_domain.into())
                        && e.needs_ping(cur_ts, opt_relay_id == Some(k))
                }) {
                    node_refs.push(NodeRef::new(
                        self.clone(),
                        k,
                        v,
                        Some(NodeRefFilter::new().with_routing_domain(routing_domain)),
                    ));
                }
                Option::<()>::None
            },
        );
        node_refs
    }

    pub fn get_all_nodes(&self, cur_ts: u64) -> Vec<NodeRef> {
        let inner = self.inner.read();
        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
        Self::with_entries(
            &*inner,
            cur_ts,
            BucketEntryState::Unreliable,
            |_rti, k, v| {
                node_refs.push(NodeRef::new(self.clone(), k, v, None));
                Option::<()>::None
            },
        );
        node_refs
    }

    fn queue_bucket_kick(&self, node_id: DHTKey) {
        let idx = self.find_bucket_index(node_id);
        self.unlocked_inner.kick_queue.lock().insert(idx);
    }

    /// Create a node reference, possibly creating a bucket entry
    /// the 'update_func' closure is called on the node, and, if created,
    /// in a locked fashion as to ensure the bucket entry state is always valid
    pub fn create_node_ref<F>(&self, node_id: DHTKey, update_func: F) -> Option<NodeRef>
    where
        F: FnOnce(&mut RoutingTableInner, &mut BucketEntryInner),
    {
        // Ensure someone isn't trying register this node itself
        if node_id == self.node_id() {
            log_rtab!(debug "can't register own node");
            return None;
        }

        // Lock this entire operation
        let mut inner = self.inner.write();
        let inner = &mut *inner;

        // Look up existing entry
        let idx = self.find_bucket_index(node_id);
        let noderef = {
            let bucket = &inner.buckets[idx];
            let entry = bucket.entry(&node_id);
            entry.map(|e| NodeRef::new(self.clone(), node_id, e, None))
        };

        // If one doesn't exist, insert into bucket, possibly evicting a bucket member
        let noderef = match noderef {
            None => {
                // Make new entry
                inner.bucket_entry_count += 1;
                let cnt = inner.bucket_entry_count;
                let bucket = &mut inner.buckets[idx];
                let nr = bucket.add_entry(node_id);

                // Update the entry
                let entry = bucket.entry(&node_id).unwrap();
                entry.with_mut(inner, update_func);

                // Kick the bucket
                self.unlocked_inner.kick_queue.lock().insert(idx);
                log_rtab!(debug "Routing table now has {} nodes, {} live", cnt, Self::get_entry_count_inner(&mut *inner, RoutingDomainSet::all(), BucketEntryState::Unreliable));

                nr
            }
            Some(nr) => {
                // Update the entry
                let bucket = &mut inner.buckets[idx];
                let entry = bucket.entry(&node_id).unwrap();
                entry.with_mut(inner, update_func);

                nr
            }
        };

        Some(noderef)
    }

    /// Resolve an existing routing table entry and return a reference to it
    fn lookup_node_ref_inner(inner: &RoutingTableInner, routing_table: RoutingTable, node_id: DHTKey) -> Option<NodeRef> {
    {
        let idx = routing_table.find_bucket_index(node_id);
        let bucket = &inner.buckets[idx];
        bucket
            .entry(&node_id)
            .map(|e| NodeRef::new(routing_table, node_id, e, None))
    }

    pub fn lookup_node_ref(&self, node_id: DHTKey) -> Option<NodeRef> {
        if node_id == self.unlocked_inner.node_id {
            log_rtab!(debug "can't look up own node id in routing table");
            return None;
        }
        let idx = self.find_bucket_index(node_id);
        let inner = self.inner.read();
        let bucket = &inner.buckets[idx];
        bucket
            .entry(&node_id)
            .map(|e| NodeRef::new(self.clone(), node_id, e, None))
    }

    /// Resolve an existing routing table entry and return a filtered reference to it
    pub fn lookup_and_filter_noderef(
        &self,
        node_id: DHTKey,
        routing_domain_set: RoutingDomainSet,
        dial_info_filter: DialInfoFilter,
    ) -> Option<NodeRef> {
        let nr = self.lookup_node_ref(node_id)?;
        Some(
            nr.filtered_clone(
                NodeRefFilter::new()
                    .with_dial_info_filter(dial_info_filter)
                    .with_routing_domain_set(routing_domain_set),
            ),
        )
    }

    /// Shortcut function to add a node to our routing table if it doesn't exist
    /// and add the dial info we have for it. Returns a noderef filtered to
    /// the routing domain in which this node was registered for convenience.
    pub fn register_node_with_signed_node_info(
        &self,
        routing_domain: RoutingDomain,
        node_id: DHTKey,
        signed_node_info: SignedNodeInfo,
        allow_invalid: bool,
    ) -> Option<NodeRef> {
        //log_rtab!("register_node_with_signed_node_info: routing_domain: {:?}, node_id: {:?}, signed_node_info: {:?}, allow_invalid: {:?}", routing_domain, node_id, signed_node_info, allow_invalid );

        // validate signed node info is not something malicious
        if node_id == self.node_id() {
            log_rtab!(debug "can't register own node id in routing table");
            return None;
        }
        if let Some(rpi) = &signed_node_info.node_info.relay_peer_info {
            if rpi.node_id.key == node_id {
                log_rtab!(debug "node can not be its own relay");
                return None;
            }
        }
        if !allow_invalid {
            // verify signature
            if !signed_node_info.has_valid_signature() {
                log_rtab!(debug "signed node info for {} has invalid signature", node_id);
                return None;
            }
            // verify signed node info is valid in this routing domain
            if !self
                .node_info_is_valid_in_routing_domain(routing_domain, &signed_node_info.node_info)
            {
                log_rtab!(debug "signed node info for {} not valid in the {:?} routing domain", node_id, routing_domain);
                return None;
            }
        }

        self.create_node_ref(node_id, |_rti, e| {
            e.update_signed_node_info(routing_domain, signed_node_info);
        })
        .map(|mut nr| {
            nr.set_filter(Some(
                NodeRefFilter::new().with_routing_domain(routing_domain),
            ));
            nr
        })
    }

    /// Shortcut function to add a node to our routing table if it doesn't exist
    /// and add the last peer address we have for it, since that's pretty common
    pub fn register_node_with_existing_connection(
        &self,
        node_id: DHTKey,
        descriptor: ConnectionDescriptor,
        timestamp: u64,
    ) -> Option<NodeRef> {
        let out = self.create_node_ref(node_id, |_rti, e| {
            // this node is live because it literally just connected to us
            e.touch_last_seen(timestamp);
        });
        if let Some(nr) = &out {
            // set the most recent node address for connection finding and udp replies
            nr.set_last_connection(descriptor, timestamp);
        }
        out
    }

    /// Ticks about once per second
    /// to run tick tasks which may run at slower tick rates as configured
    pub async fn tick(&self) -> EyreResult<()> {
        // Do rolling transfers every ROLLING_TRANSFERS_INTERVAL_SECS secs
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // Kick buckets task
        let kick_bucket_queue_count = self.unlocked_inner.kick_queue.lock().len();
        if kick_bucket_queue_count > 0 {
            self.unlocked_inner.kick_buckets_task.tick().await?;
        }

        Ok(())
    }

    //////////////////////////////////////////////////////////////////////
    // Routing Table Health Metrics

    pub fn get_routing_table_health(&self) -> RoutingTableHealth {
        let mut health = RoutingTableHealth::default();
        let cur_ts = intf::get_timestamp();
        let inner = self.inner.read();
        let inner = &*inner;
        for bucket in &inner.buckets {
            for (_, v) in bucket.entries() {
                match v.with(inner, |_rti, e| e.state(cur_ts)) {
                    BucketEntryState::Reliable => {
                        health.reliable_entry_count += 1;
                    }
                    BucketEntryState::Unreliable => {
                        health.unreliable_entry_count += 1;
                    }
                    BucketEntryState::Dead => {
                        health.dead_entry_count += 1;
                    }
                }
            }
        }
        health
    }

    pub fn get_recent_peers(&self) -> Vec<(DHTKey, RecentPeersEntry)> {
        let mut recent_peers = Vec::new();
        let mut dead_peers = Vec::new();
        let mut out = Vec::new();

        // collect all recent peers
        {
            let inner = self.inner.read();
            for (k, _v) in &inner.recent_peers {
                recent_peers.push(*k);
            }
        }

        // look up each node and make sure the connection is still live
        // (uses same logic as send_data, ensuring last_connection works for UDP)
        for e in &recent_peers {
            let mut dead = true;
            if let Some(nr) = self.lookup_node_ref(*e) {
                if let Some(last_connection) = nr.last_connection() {
                    out.push((*e, RecentPeersEntry { last_connection }));
                    dead = false;
                }
            }
            if dead {
                dead_peers.push(e);
            }
        }

        // purge dead recent peers
        {
            let mut inner = self.inner.write();
            for d in dead_peers {
                inner.recent_peers.remove(d);
            }
        }

        out
    }

    pub fn touch_recent_peer(&self, node_id: DHTKey, last_connection: ConnectionDescriptor) {
        let mut inner = self.inner.write();
        inner
            .recent_peers
            .insert(node_id, RecentPeersEntry { last_connection });
    }
}
