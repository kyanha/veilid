mod bucket;
mod bucket_entry;
mod debug;
mod find_nodes;
mod node_ref;
mod stats_accounting;

use crate::dht::*;
use crate::intf::*;
use crate::network_manager::*;
use crate::rpc_processor::*;
use crate::xx::*;
use crate::*;
use alloc::str::FromStr;
use bucket::*;
pub use bucket_entry::*;
pub use debug::*;
pub use find_nodes::*;
use futures_util::stream::{FuturesUnordered, StreamExt};
pub use node_ref::*;
pub use stats_accounting::*;

//////////////////////////////////////////////////////////////////////////

pub const BOOTSTRAP_TXT_VERSION: u8 = 0;

#[derive(Clone, Debug)]
pub struct BootstrapRecord {
    min_version: u8,
    max_version: u8,
    dial_info_details: Vec<DialInfoDetail>,
}
pub type BootstrapRecordMap = BTreeMap<DHTKey, BootstrapRecord>;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Ord, Eq)]
pub enum RoutingDomain {
    PublicInternet,
    LocalNetwork,
}

#[derive(Debug, Default)]
pub struct RoutingDomainDetail {
    dial_info_details: Vec<DialInfoDetail>,
}

struct RoutingTableInner {
    network_manager: NetworkManager,
    node_id: DHTKey,
    node_id_secret: DHTKeySecret,
    buckets: Vec<Bucket>,
    public_internet_routing_domain: RoutingDomainDetail,
    local_network_routing_domain: RoutingDomainDetail,
    bucket_entry_count: usize,

    // Transfer stats for this node
    self_latency_stats_accounting: LatencyStatsAccounting,
    self_transfer_stats_accounting: TransferStatsAccounting,
    self_transfer_stats: TransferStatsDownUp,
}

#[derive(Clone, Debug, Default)]
pub struct RoutingTableHealth {
    pub reliable_entry_count: usize,
    pub unreliable_entry_count: usize,
    pub dead_entry_count: usize,
}

struct RoutingTableUnlockedInner {
    // Background processes
    rolling_transfers_task: TickTask,
    bootstrap_task: TickTask,
    peer_minimum_refresh_task: TickTask,
    ping_validator_task: TickTask,
    node_info_update_single_future: SingleFuture<()>,
}

#[derive(Clone)]
pub struct RoutingTable {
    config: VeilidConfig,
    inner: Arc<Mutex<RoutingTableInner>>,
    unlocked_inner: Arc<RoutingTableUnlockedInner>,
}

impl RoutingTable {
    fn new_inner(network_manager: NetworkManager) -> RoutingTableInner {
        RoutingTableInner {
            network_manager,
            node_id: DHTKey::default(),
            node_id_secret: DHTKeySecret::default(),
            buckets: Vec::new(),
            public_internet_routing_domain: RoutingDomainDetail::default(),
            local_network_routing_domain: RoutingDomainDetail::default(),
            bucket_entry_count: 0,
            self_latency_stats_accounting: LatencyStatsAccounting::new(),
            self_transfer_stats_accounting: TransferStatsAccounting::new(),
            self_transfer_stats: TransferStatsDownUp::default(),
        }
    }
    fn new_unlocked_inner(config: VeilidConfig) -> RoutingTableUnlockedInner {
        let c = config.get();
        RoutingTableUnlockedInner {
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
            bootstrap_task: TickTask::new(1),
            peer_minimum_refresh_task: TickTask::new_ms(c.network.dht.min_peer_refresh_time_ms),
            ping_validator_task: TickTask::new(1),
            node_info_update_single_future: SingleFuture::new(),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        let config = network_manager.config();
        let this = Self {
            config: config.clone(),
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
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
        // Set bootstrap tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .bootstrap_task
                .set_routine(move |_l, _t| Box::pin(this2.clone().bootstrap_task_routine()));
        }
        // Set peer minimum refresh tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .peer_minimum_refresh_task
                .set_routine(move |_l, _t| {
                    Box::pin(this2.clone().peer_minimum_refresh_task_routine())
                });
        }
        // Set ping validator tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .ping_validator_task
                .set_routine(move |l, t| Box::pin(this2.clone().ping_validator_task_routine(l, t)));
        }
        this
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }
    pub fn rpc_processor(&self) -> RPCProcessor {
        self.network_manager().rpc_processor()
    }

    pub fn node_id(&self) -> DHTKey {
        self.inner.lock().node_id
    }

    pub fn node_id_secret(&self) -> DHTKeySecret {
        self.inner.lock().node_id_secret
    }

    fn with_routing_domain<F, R>(inner: &RoutingTableInner, domain: RoutingDomain, f: F) -> R
    where
        F: FnOnce(&RoutingDomainDetail) -> R,
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
        F: FnOnce(&mut RoutingDomainDetail) -> R,
    {
        match domain {
            RoutingDomain::PublicInternet => f(&mut inner.public_internet_routing_domain),
            RoutingDomain::LocalNetwork => f(&mut inner.local_network_routing_domain),
        }
    }

    pub fn has_dial_info(&self, domain: RoutingDomain) -> bool {
        let inner = self.inner.lock();
        Self::with_routing_domain(&*inner, domain, |rd| !rd.dial_info_details.is_empty())
    }

    pub fn dial_info_details(&self, domain: RoutingDomain) -> Vec<DialInfoDetail> {
        let inner = self.inner.lock();
        Self::with_routing_domain(&*inner, domain, |rd| rd.dial_info_details.clone())
    }

    pub fn first_filtered_dial_info_detail(
        &self,
        domain: Option<RoutingDomain>,
        filter: &DialInfoFilter,
    ) -> Option<DialInfoDetail> {
        let inner = self.inner.lock();
        // Prefer local network first if it isn't filtered out
        if domain == None || domain == Some(RoutingDomain::LocalNetwork) {
            Self::with_routing_domain(&*inner, RoutingDomain::LocalNetwork, |rd| {
                for did in &rd.dial_info_details {
                    if did.matches_filter(filter) {
                        return Some(did.clone());
                    }
                }
                None
            })
        } else {
            None
        }
        .or_else(|| {
            if domain == None || domain == Some(RoutingDomain::PublicInternet) {
                Self::with_routing_domain(&*inner, RoutingDomain::PublicInternet, |rd| {
                    for did in &rd.dial_info_details {
                        if did.matches_filter(filter) {
                            return Some(did.clone());
                        }
                    }
                    None
                })
            } else {
                None
            }
        })
    }

    pub fn all_filtered_dial_info_details(
        &self,
        domain: Option<RoutingDomain>,
        filter: &DialInfoFilter,
    ) -> Vec<DialInfoDetail> {
        let inner = self.inner.lock();
        let mut ret = Vec::new();

        if domain == None || domain == Some(RoutingDomain::LocalNetwork) {
            Self::with_routing_domain(&*inner, RoutingDomain::LocalNetwork, |rd| {
                for did in &rd.dial_info_details {
                    if did.matches_filter(filter) {
                        ret.push(did.clone());
                    }
                }
            });
        }
        if domain == None || domain == Some(RoutingDomain::PublicInternet) {
            Self::with_routing_domain(&*inner, RoutingDomain::PublicInternet, |rd| {
                for did in &rd.dial_info_details {
                    if did.matches_filter(filter) {
                        ret.push(did.clone());
                    }
                }
            });
        }
        ret.remove_duplicates();
        ret
    }

    pub fn register_dial_info(
        &self,
        domain: RoutingDomain,
        dial_info: DialInfo,
        class: DialInfoClass,
    ) -> Result<(), String> {
        log_rtab!(debug
            "Registering dial_info with:\n  domain: {:?}\n  dial_info: {:?}\n  class: {:?}",
            domain, dial_info, class
        );
        let enable_local_peer_scope = {
            let config = self.network_manager().config();
            let c = config.get();
            c.network.enable_local_peer_scope
        };

        if !enable_local_peer_scope
            && matches!(domain, RoutingDomain::PublicInternet)
            && dial_info.is_local()
        {
            return Err("shouldn't be registering local addresses as public".to_owned())
                .map_err(logthru_rtab!(error));
        }
        if !dial_info.is_valid() {
            return Err(format!(
                "shouldn't be registering invalid addresses: {:?}",
                dial_info
            ))
            .map_err(logthru_rtab!(error));
        }

        let mut inner = self.inner.lock();
        Self::with_routing_domain_mut(&mut *inner, domain, |rd| {
            rd.dial_info_details.push(DialInfoDetail {
                dial_info: dial_info.clone(),
                class,
            });
            rd.dial_info_details.sort();
        });

        let domain_str = match domain {
            RoutingDomain::PublicInternet => "Public",
            RoutingDomain::LocalNetwork => "Local",
        };
        info!(
            "{} Dial Info: {}",
            domain_str,
            NodeDialInfo {
                node_id: NodeId::new(inner.node_id),
                dial_info
            }
            .to_string(),
        );
        debug!("    Class: {:?}", class);

        // Public dial info changed, go through all nodes and reset their 'seen our node info' bit
        if matches!(domain, RoutingDomain::PublicInternet) {
            let cur_ts = intf::get_timestamp();
            Self::with_entries(&mut *inner, cur_ts, BucketEntryState::Dead, |_, e| {
                e.set_seen_our_node_info(false);
                Option::<()>::None
            });
        }

        Ok(())
    }

    pub fn clear_dial_info_details(&self, domain: RoutingDomain) {
        trace!("clearing dial info domain: {:?}", domain);

        let mut inner = self.inner.lock();
        Self::with_routing_domain_mut(&mut *inner, domain, |rd| {
            rd.dial_info_details.clear();
        })
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

    pub async fn init(&self) -> Result<(), String> {
        let mut inner = self.inner.lock();
        // Size the buckets (one per bit)
        inner.buckets.reserve(DHT_KEY_LENGTH * 8);
        for _ in 0..DHT_KEY_LENGTH * 8 {
            let bucket = Bucket::new(self.clone());
            inner.buckets.push(bucket);
        }

        // make local copy of node id for easy access
        let c = self.config.get();
        inner.node_id = c.network.node_id;
        inner.node_id_secret = c.network.node_id_secret;

        Ok(())
    }

    pub async fn terminate(&self) {
        // Cancel all tasks being ticked
        if let Err(e) = self.unlocked_inner.rolling_transfers_task.cancel().await {
            warn!("rolling_transfers_task not cancelled: {}", e);
        }
        if let Err(e) = self.unlocked_inner.bootstrap_task.cancel().await {
            warn!("bootstrap_task not cancelled: {}", e);
        }
        if let Err(e) = self.unlocked_inner.peer_minimum_refresh_task.cancel().await {
            warn!("peer_minimum_refresh_task not cancelled: {}", e);
        }
        if let Err(e) = self.unlocked_inner.ping_validator_task.cancel().await {
            warn!("ping_validator_task not cancelled: {}", e);
        }
        if self
            .unlocked_inner
            .node_info_update_single_future
            .cancel()
            .await
            .is_err()
        {
            warn!("node_info_update_single_future not cancelled");
        }

        *self.inner.lock() = Self::new_inner(self.network_manager());
    }

    // Inform routing table entries that our dial info has changed
    pub fn send_node_info_updates(&self) {
        let this = self.clone();
        // Run in background
        intf::spawn(async move {
            // Run in background only once
            this.clone()
                .unlocked_inner
                .node_info_update_single_future
                .single_spawn(async move {

                    // Only update if we actually have a valid network class
                    let netman = this.network_manager();
                    if matches!(
                        netman.get_network_class().unwrap_or(NetworkClass::Invalid),
                        NetworkClass::Invalid
                    ) {
                        trace!("not sending node info update because our network class is not yet valid");
                        return;
                    }

                    // Get the list of refs to all nodes to update
                    let node_refs = {
                        let mut inner = this.inner.lock();
                        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
                        let cur_ts = intf::get_timestamp();
                        Self::with_entries(&mut *inner, cur_ts, BucketEntryState::Unreliable, |k, e| {
                            // Only update nodes that haven't seen our node info yet
                            if !e.has_seen_our_node_info() {
                                node_refs.push(NodeRef::new(
                                    this.clone(),
                                    *k,
                                    e,
                                    None,
                                ));
                            }
                            Option::<()>::None
                        });
                        node_refs
                    };

                    // Send the updates
                    log_rtab!("Sending node info updates to {} nodes", node_refs.len());
                    let mut unord = FuturesUnordered::new();
                    for nr in node_refs {
                        let rpc = this.rpc_processor();
                        unord.push(async move {
                            // Update the node
                            if let Err(e) = rpc
                                .rpc_call_node_info_update(Destination::Direct(nr.clone()), None)
                                .await
                            {
                                // Not fatal, but we should be able to see if this is happening
                                trace!("failed to send node info update to {:?}: {}", nr, e);
                                return;
                            }

                            // Mark the node as updated
                            nr.set_seen_our_node_info();
                        });
                    }

                    // Wait for futures to complete
                    while unord.next().await.is_some() {}

                    log_rtab!("Finished sending node updates");
                })
                .await
        })
        .detach()
    }

    // Attempt to empty the routing table
    // should only be performed when there are no node_refs (detached)
    pub fn purge(&self) {
        let mut inner = self.inner.lock();
        log_rtab!(
            "Starting routing table purge. Table currently has {} nodes",
            inner.bucket_entry_count
        );
        for bucket in &mut inner.buckets {
            bucket.kick(0);
        }
        log_rtab!(debug
             "Routing table purge complete. Routing table now has {} nodes",
            inner.bucket_entry_count
        );
    }

    // Attempt to settle buckets and remove entries down to the desired number
    // which may not be possible due extant NodeRefs
    fn kick_bucket(inner: &mut RoutingTableInner, idx: usize) {
        let bucket = &mut inner.buckets[idx];
        let bucket_depth = Self::bucket_depth(idx);

        if let Some(dead_node_ids) = bucket.kick(bucket_depth) {
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

    fn find_bucket_index(inner: &RoutingTableInner, node_id: DHTKey) -> usize {
        distance(&node_id, &inner.node_id)
            .first_nonzero_bit()
            .unwrap()
    }

    fn get_entry_count(inner: &mut RoutingTableInner, min_state: BucketEntryState) -> usize {
        let mut count = 0usize;
        let cur_ts = intf::get_timestamp();
        Self::with_entries(inner, cur_ts, min_state, |_, _| {
            count += 1;
            Option::<()>::None
        });
        count
    }

    fn with_entries<T, F: FnMut(&DHTKey, &mut BucketEntry) -> Option<T>>(
        inner: &mut RoutingTableInner,
        cur_ts: u64,
        min_state: BucketEntryState,
        mut f: F,
    ) -> Option<T> {
        for bucket in &mut inner.buckets {
            for entry in bucket.entries_mut() {
                if entry.1.state(cur_ts) >= min_state {
                    if let Some(out) = f(entry.0, entry.1) {
                        return Some(out);
                    }
                }
            }
        }
        None
    }

    fn drop_node_ref(&self, node_id: DHTKey) {
        // Reduce ref count on entry
        let mut inner = self.inner.lock();
        let idx = Self::find_bucket_index(&*inner, node_id);
        let new_ref_count = {
            let bucket = &mut inner.buckets[idx];
            let entry = bucket.entry_mut(&node_id).unwrap();
            entry.ref_count -= 1;
            entry.ref_count
        };

        // If this entry could possibly go away, kick the bucket
        if new_ref_count == 0 {
            // it important to do this in the same inner lock as the ref count decrease
            Self::kick_bucket(&mut *inner, idx);
        }
    }

    // Create a node reference, possibly creating a bucket entry
    // the 'update_func' closure is called on the node, and, if created,
    // in a locked fashion as to ensure the bucket entry state is always valid
    pub fn create_node_ref<F>(&self, node_id: DHTKey, update_func: F) -> Result<NodeRef, String>
    where
        F: FnOnce(&mut BucketEntry),
    {
        // Ensure someone isn't trying register this node itself
        if node_id == self.node_id() {
            return Err("can't register own node".to_owned()).map_err(logthru_rtab!(error));
        }

        // Lock this entire operation
        let mut inner = self.inner.lock();

        // Look up existing entry
        let idx = Self::find_bucket_index(&*inner, node_id);
        let noderef = {
            let bucket = &mut inner.buckets[idx];
            let entry = bucket.entry_mut(&node_id);
            entry.map(|e| NodeRef::new(self.clone(), node_id, e, None))
        };

        // If one doesn't exist, insert into bucket, possibly evicting a bucket member
        let noderef = match noderef {
            None => {
                // Make new entry
                inner.bucket_entry_count += 1;
                let cnt = inner.bucket_entry_count;
                log_rtab!(debug "Routing table now has {} nodes, {} live", cnt, Self::get_entry_count(&mut *inner, BucketEntryState::Unreliable));
                let bucket = &mut inner.buckets[idx];
                let nr = bucket.add_entry(node_id);

                // Update the entry
                let entry = bucket.entry_mut(&node_id);
                update_func(entry.unwrap());

                // Kick the bucket
                // It is important to do this in the same inner lock as the add_entry
                Self::kick_bucket(&mut *inner, idx);

                nr
            }
            Some(nr) => {
                // Update the entry
                let bucket = &mut inner.buckets[idx];
                let entry = bucket.entry_mut(&node_id);
                update_func(entry.unwrap());

                nr
            }
        };

        Ok(noderef)
    }

    pub fn lookup_node_ref(&self, node_id: DHTKey) -> Option<NodeRef> {
        let mut inner = self.inner.lock();
        let idx = Self::find_bucket_index(&*inner, node_id);
        let bucket = &mut inner.buckets[idx];
        bucket
            .entry_mut(&node_id)
            .map(|e| NodeRef::new(self.clone(), node_id, e, None))
    }

    // Shortcut function to add a node to our routing table if it doesn't exist
    // and add the dial info we have for it, since that's pretty common
    pub fn register_node_with_signed_node_info(
        &self,
        node_id: DHTKey,
        signed_node_info: SignedNodeInfo,
    ) -> Result<NodeRef, String> {
        // validate signed node info is not something malicious
        if node_id == self.node_id() {
            return Err("can't register own node id in routing table".to_owned());
        }
        if let Some(rpi) = &signed_node_info.node_info.relay_peer_info {
            if rpi.node_id.key == node_id {
                return Err("node can not be its own relay".to_owned());
            }
        }

        let nr = self.create_node_ref(node_id, |e| {
            e.update_node_info(signed_node_info);
        })?;

        Ok(nr)
    }

    // Shortcut function to add a node to our routing table if it doesn't exist
    // and add the last peer address we have for it, since that's pretty common
    pub fn register_node_with_existing_connection(
        &self,
        node_id: DHTKey,
        descriptor: ConnectionDescriptor,
        timestamp: u64,
    ) -> Result<NodeRef, String> {
        let nr = self.create_node_ref(node_id, |e| {
            // set the most recent node address for connection finding and udp replies
            e.set_last_connection(descriptor, timestamp);
        })?;

        Ok(nr)
    }

    fn operate_on_bucket_entry_locked<T, F>(
        inner: &mut RoutingTableInner,
        node_id: DHTKey,
        f: F,
    ) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        let idx = Self::find_bucket_index(&*inner, node_id);
        let bucket = &mut inner.buckets[idx];
        let entry = bucket.entry_mut(&node_id).unwrap();
        f(entry)
    }

    fn operate_on_bucket_entry<T, F>(&self, node_id: DHTKey, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        let mut inner = self.inner.lock();
        Self::operate_on_bucket_entry_locked(&mut *inner, node_id, f)
    }

    pub fn find_inbound_relay(&self, cur_ts: u64) -> Option<NodeRef> {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;
        let mut best_inbound_relay: Option<(&DHTKey, &mut BucketEntry)> = None;

        // Iterate all known nodes for candidates
        for bucket in &mut inner.buckets {
            for (k, e) in bucket.entries_mut() {
                if e.state(cur_ts) >= BucketEntryState::Unreliable {
                    // Ensure this node is not on our local network
                    if !e
                        .local_node_info()
                        .map(|l| l.has_dial_info())
                        .unwrap_or(false)
                    {
                        // Ensure we have the node's status
                        if let Some(node_status) = &e.peer_stats().status {
                            // Ensure the node will relay
                            if node_status.will_relay {
                                // Compare against previous candidate
                                if let Some(best_inbound_relay) = best_inbound_relay.as_mut() {
                                    // Less is faster
                                    if BucketEntry::cmp_fastest_reliable(
                                        cur_ts,
                                        e,
                                        best_inbound_relay.1,
                                    ) == std::cmp::Ordering::Less
                                    {
                                        *best_inbound_relay = (k, e);
                                    }
                                } else {
                                    // Always store the first candidate
                                    best_inbound_relay = Some((k, e));
                                }
                            }
                        }
                    }
                }
            }
        }
        // Return the best inbound relay noderef
        best_inbound_relay.map(|(k, e)| NodeRef::new(self.clone(), *k, e, None))
    }

    #[instrument(level = "trace", skip(self), ret, err)]
    pub fn register_find_node_answer(&self, fna: FindNodeAnswer) -> Result<Vec<NodeRef>, String> {
        let node_id = self.node_id();

        // register nodes we'd found
        let mut out = Vec::<NodeRef>::with_capacity(fna.peers.len());
        for p in fna.peers {
            // if our own node if is in the list then ignore it, as we don't add ourselves to our own routing table
            if p.node_id.key == node_id {
                continue;
            }

            // register the node if it's new
            let nr = self
                .register_node_with_signed_node_info(p.node_id.key, p.signed_node_info.clone())
                .map_err(map_to_string)
                .map_err(logthru_rtab!(
                    "couldn't register node {} at {:?}",
                    p.node_id.key,
                    &p.signed_node_info
                ))?;
            out.push(nr);
        }
        Ok(out)
    }

    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn find_node(
        &self,
        node_ref: NodeRef,
        node_id: DHTKey,
    ) -> Result<Vec<NodeRef>, String> {
        let rpc_processor = self.rpc_processor();

        let res = rpc_processor
            .clone()
            .rpc_call_find_node(
                Destination::Direct(node_ref.clone()),
                node_id,
                None,
                rpc_processor.make_respond_to_sender(node_ref.clone()),
            )
            .await
            .map_err(map_to_string)
            .map_err(logthru_rtab!())?;

        // register nodes we'd found
        self.register_find_node_answer(res)
    }

    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn find_self(&self, node_ref: NodeRef) -> Result<Vec<NodeRef>, String> {
        let node_id = self.node_id();
        self.find_node(node_ref, node_id).await
    }

    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn find_target(&self, node_ref: NodeRef) -> Result<Vec<NodeRef>, String> {
        let node_id = node_ref.node_id();
        self.find_node(node_ref, node_id).await
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn reverse_find_node(&self, node_ref: NodeRef, wide: bool) {
        // Ask bootstrap node to 'find' our own node so we can get some more nodes near ourselves
        // and then contact those nodes to inform -them- that we exist

        // Ask bootstrap server for nodes closest to our own node
        let closest_nodes = match self.find_self(node_ref.clone()).await {
            Err(e) => {
                log_rtab!(error
                    "reverse_find_node: find_self failed for {:?}: {}",
                    &node_ref, e
                );
                return;
            }
            Ok(v) => v,
        };

        // Ask each node near us to find us as well
        if wide {
            for closest_nr in closest_nodes {
                match self.find_self(closest_nr.clone()).await {
                    Err(e) => {
                        log_rtab!(error
                            "reverse_find_node: closest node find_self failed for {:?}: {}",
                            &closest_nr, e
                        );
                        return;
                    }
                    Ok(v) => v,
                };
            }
        }
    }

    // Bootstrap lookup process
    #[instrument(level = "trace", skip(self), ret, err)]
    async fn resolve_bootstrap(
        &self,
        bootstrap: Vec<String>,
    ) -> Result<BootstrapRecordMap, String> {
        // Resolve from bootstrap root to bootstrap hostnames
        let mut bsnames = Vec::<String>::new();
        for bh in bootstrap {
            // Get TXT record for bootstrap (bootstrap.veilid.net, or similar)
            let records = intf::txt_lookup(&bh).await?;
            for record in records {
                // Split the bootstrap name record by commas
                for rec in record.split(',') {
                    let rec = rec.trim();
                    // If the name specified is fully qualified, go with it
                    let bsname = if rec.ends_with('.') {
                        rec.to_string()
                    }
                    // If the name is not fully qualified, prepend it to the bootstrap name
                    else {
                        format!("{}.{}", rec, bh)
                    };

                    // Add to the list of bootstrap name to look up
                    bsnames.push(bsname);
                }
            }
        }

        // Get bootstrap nodes from hostnames concurrently
        let mut unord = FuturesUnordered::new();
        for bsname in bsnames {
            unord.push(async move {
                // look up boostrap node txt records
                let bsnirecords = match intf::txt_lookup(&bsname).await {
                    Err(e) => {
                        warn!("bootstrap node txt lookup failed for {}: {}", bsname, e);
                        return None;
                    }
                    Ok(v) => v,
                };
                // for each record resolve into key/bootstraprecord pairs
                let mut bootstrap_records: Vec<(DHTKey, BootstrapRecord)> = Vec::new();
                for bsnirecord in bsnirecords {
                    // Bootstrap TXT Record Format Version 0:
                    // txt_version,min_version,max_version,nodeid,hostname,dialinfoshort*
                    //
                    // Split bootstrap node record by commas. Example:
                    // 0,0,0,7lxDEabK_qgjbe38RtBa3IZLrud84P6NhGP-pRTZzdQ,bootstrap-dev-alpha.veilid.net,T5150,U5150,W5150/ws
                    let records: Vec<String> = bsnirecord
                        .trim()
                        .split(',')
                        .map(|x| x.trim().to_owned())
                        .collect();
                    if records.len() < 6 {
                        warn!("invalid number of fields in bootstrap txt record");
                        continue;
                    }

                    // Bootstrap TXT record version
                    let txt_version: u8 = match records[0].parse::<u8>() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "invalid txt_version specified in bootstrap node txt record: {}",
                                e
                            );
                            continue;
                        }
                    };
                    if txt_version != BOOTSTRAP_TXT_VERSION {
                        warn!("unsupported bootstrap txt record version");
                        continue;
                    }

                    // Min/Max wire protocol version
                    let min_version: u8 = match records[1].parse::<u8>() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "invalid min_version specified in bootstrap node txt record: {}",
                                e
                            );
                            continue;
                        }
                    };
                    let max_version: u8 = match records[2].parse::<u8>() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "invalid max_version specified in bootstrap node txt record: {}",
                                e
                            );
                            continue;
                        }
                    };

                    // Node Id
                    let node_id_str = &records[3];
                    let node_id_key = match DHTKey::try_decode(node_id_str) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "Invalid node id in bootstrap node record {}: {}",
                                node_id_str, e
                            );
                            continue;
                        }
                    };

                    // Hostname
                    let hostname_str = &records[4];

                    // If this is our own node id, then we skip it for bootstrap, in case we are a bootstrap node
                    if self.node_id() == node_id_key {
                        continue;
                    }

                    // Resolve each record and store in node dial infos list
                    let mut bootstrap_record = BootstrapRecord {
                        min_version,
                        max_version,
                        dial_info_details: Vec::new(),
                    };
                    for rec in &records[5..] {
                        let rec = rec.trim();
                        let dial_infos = match DialInfo::try_vec_from_short(rec, hostname_str) {
                            Ok(dis) => dis,
                            Err(e) => {
                                warn!("Couldn't resolve bootstrap node dial info {}: {}", rec, e);
                                continue;
                            }
                        };

                        for di in dial_infos {
                            bootstrap_record.dial_info_details.push(DialInfoDetail {
                                dial_info: di,
                                class: DialInfoClass::Direct,
                            });
                        }
                    }
                    bootstrap_records.push((node_id_key, bootstrap_record));
                }
                Some(bootstrap_records)
            });
        }

        let mut bsmap = BootstrapRecordMap::new();
        while let Some(bootstrap_records) = unord.next().await {
            if let Some(bootstrap_records) = bootstrap_records {
                for (bskey, mut bsrec) in bootstrap_records {
                    let rec = bsmap.entry(bskey).or_insert_with(|| BootstrapRecord {
                        min_version: bsrec.min_version,
                        max_version: bsrec.max_version,
                        dial_info_details: Vec::new(),
                    });
                    rec.dial_info_details.append(&mut bsrec.dial_info_details);
                }
            }
        }

        Ok(bsmap)
    }

    #[instrument(level = "trace", skip(self), err)]
    async fn bootstrap_task_routine(self) -> Result<(), String> {
        let (bootstrap, bootstrap_nodes) = {
            let c = self.config.get();
            (
                c.network.bootstrap.clone(),
                c.network.bootstrap_nodes.clone(),
            )
        };

        log_rtab!(debug "--- bootstrap_task");

        // If we aren't specifying a bootstrap node list explicitly, then pull from the bootstrap server(s)

        let bsmap: BootstrapRecordMap = if !bootstrap_nodes.is_empty() {
            let mut bsmap = BootstrapRecordMap::new();
            let mut bootstrap_node_dial_infos = Vec::new();
            for b in bootstrap_nodes {
                let ndis = NodeDialInfo::from_str(b.as_str())
                    .map_err(map_to_string)
                    .map_err(logthru_rtab!(
                        "Invalid node dial info in bootstrap entry: {}",
                        b
                    ))?;
                bootstrap_node_dial_infos.push(ndis);
            }
            for ndi in bootstrap_node_dial_infos {
                let node_id = ndi.node_id.key;
                bsmap
                    .entry(node_id)
                    .or_insert_with(|| BootstrapRecord {
                        min_version: MIN_VERSION,
                        max_version: MAX_VERSION,
                        dial_info_details: Vec::new(),
                    })
                    .dial_info_details
                    .push(DialInfoDetail {
                        dial_info: ndi.dial_info,
                        class: DialInfoClass::Direct, // Bootstraps are always directly reachable
                    });
            }
            bsmap
        } else {
            // Resolve bootstrap servers and recurse their TXT entries
            self.resolve_bootstrap(bootstrap).await?
        };

        // Map all bootstrap entries to a single key with multiple dialinfo

        // Run all bootstrap operations concurrently
        let mut unord = FuturesUnordered::new();
        for (k, mut v) in bsmap {
            // Sort dial info so we get the preferred order correct
            v.dial_info_details.sort();

            log_rtab!("--- bootstrapping {} with {:?}", k.encode(), &v);

            // Make invalid signed node info (no signature)
            let nr = self
                .register_node_with_signed_node_info(
                    k,
                    SignedNodeInfo::with_no_signature(NodeInfo {
                        network_class: NetworkClass::InboundCapable, // Bootstraps are always inbound capable
                        outbound_protocols: ProtocolSet::empty(), // Bootstraps do not participate in relaying and will not make outbound requests
                        min_version: v.min_version, // Minimum protocol version specified in txt record
                        max_version: v.max_version, // Maximum protocol version specified in txt record
                        dial_info_detail_list: v.dial_info_details, // Dial info is as specified in the bootstrap list
                        relay_peer_info: None, // Bootstraps never require a relay themselves
                    }),
                )
                .map_err(logthru_rtab!(error "Couldn't add bootstrap node: {}", k))?;

            // Add this our futures to process in parallel
            let this = self.clone();
            unord.push(async move {
                // Need VALID signed peer info, so ask bootstrap to find_node of itself
                // which will ensure it has the bootstrap's signed peer info as part of the response
                let _ = this.find_target(nr.clone()).await;

                // Ensure we got the signed peer info
                if !nr.operate(|e| e.has_valid_signed_node_info()) {
                    log_rtab!(warn
                        "bootstrap at {:?} did not return valid signed node info",
                        nr
                    );
                    // If this node info is invalid, it will time out after being unpingable
                } else {
                    // otherwise this bootstrap is valid, lets ask it to find ourselves now
                    this.reverse_find_node(nr, true).await
                }
            });
        }

        // Wait for all bootstrap operations to complete before we complete the singlefuture
        while unord.next().await.is_some() {}
        Ok(())
    }

    ///////////////////////////////////////////////////////////
    /// Peer ping validation

    // Ask our remaining peers to give us more peers before we go
    // back to the bootstrap servers to keep us from bothering them too much
    #[instrument(level = "trace", skip(self), err)]
    async fn peer_minimum_refresh_task_routine(self) -> Result<(), String> {
        // get list of all peers we know about, even the unreliable ones, and ask them to find nodes close to our node too
        let noderefs = {
            let mut inner = self.inner.lock();
            let mut noderefs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
            let cur_ts = intf::get_timestamp();
            Self::with_entries(
                &mut *inner,
                cur_ts,
                BucketEntryState::Unreliable,
                |k, entry| {
                    noderefs.push(NodeRef::new(self.clone(), *k, entry, None));
                    Option::<()>::None
                },
            );
            noderefs
        };

        // do peer minimum search concurrently
        let mut unord = FuturesUnordered::new();
        for nr in noderefs {
            log_rtab!("--- peer minimum search with {:?}", nr);
            unord.push(self.reverse_find_node(nr, false));
        }
        while unord.next().await.is_some() {}

        Ok(())
    }

    // Ping each node in the routing table if they need to be pinged
    // to determine their reliability
    #[instrument(level = "trace", skip(self), err)]
    async fn ping_validator_task_routine(self, _last_ts: u64, cur_ts: u64) -> Result<(), String> {
        // log_rtab!("--- ping_validator task");

        let rpc = self.rpc_processor();
        let netman = self.network_manager();
        let relay_node_id = netman.relay_node().map(|nr| nr.node_id());

        let mut inner = self.inner.lock();
        Self::with_entries(&mut *inner, cur_ts, BucketEntryState::Unreliable, |k, e| {
            if e.needs_ping(k, cur_ts, relay_node_id) {
                let nr = NodeRef::new(self.clone(), *k, e, None);
                log_rtab!(
                    "    --- ping validating: {:?} ({})",
                    nr,
                    e.state_debug_info(cur_ts)
                );
                intf::spawn_local(rpc.clone().rpc_call_status(nr)).detach();
            }
            Option::<()>::None
        });
        Ok(())
    }

    // Compute transfer statistics to determine how 'fast' a node is
    #[instrument(level = "trace", skip(self), err)]
    async fn rolling_transfers_task_routine(self, last_ts: u64, cur_ts: u64) -> Result<(), String> {
        // log_rtab!("--- rolling_transfers task");
        let inner = &mut *self.inner.lock();

        // Roll our own node's transfers
        inner.self_transfer_stats_accounting.roll_transfers(
            last_ts,
            cur_ts,
            &mut inner.self_transfer_stats,
        );

        // Roll all bucket entry transfers
        for b in &mut inner.buckets {
            b.roll_transfers(last_ts, cur_ts);
        }
        Ok(())
    }

    // Ticks about once per second
    // to run tick tasks which may run at slower tick rates as configured
    pub async fn tick(&self) -> Result<(), String> {
        // Do rolling transfers every ROLLING_TRANSFERS_INTERVAL_SECS secs
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // If routing table has no live entries, then add the bootstrap nodes to it
        if Self::get_entry_count(&mut *self.inner.lock(), BucketEntryState::Unreliable) == 0 {
            self.unlocked_inner.bootstrap_task.tick().await?;
        }

        // If we still don't have enough peers, find nodes until we do
        let min_peer_count = {
            let c = self.config.get();
            c.network.dht.min_peer_count as usize
        };
        if Self::get_entry_count(&mut *self.inner.lock(), BucketEntryState::Unreliable)
            < min_peer_count
        {
            self.unlocked_inner.peer_minimum_refresh_task.tick().await?;
        }
        // Ping validate some nodes to groom the table
        self.unlocked_inner.ping_validator_task.tick().await?;

        // Keepalive

        Ok(())
    }

    //////////////////////////////////////////////////////////////////////
    // Stats Accounting
    pub fn stats_question_sent(
        &self,
        node_ref: NodeRef,
        ts: u64,
        bytes: u64,
        expects_answer: bool,
    ) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_up(bytes);
        node_ref.operate(|e| {
            e.question_sent(ts, bytes, expects_answer);
        })
    }
    pub fn stats_question_rcvd(&self, node_ref: NodeRef, ts: u64, bytes: u64) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_down(bytes);
        node_ref.operate(|e| {
            e.question_rcvd(ts, bytes);
        })
    }
    pub fn stats_answer_sent(&self, node_ref: NodeRef, bytes: u64) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_up(bytes);
        node_ref.operate(|e| {
            e.answer_sent(bytes);
        })
    }
    pub fn stats_answer_rcvd(&self, node_ref: NodeRef, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_down(bytes);
        self.inner
            .lock()
            .self_latency_stats_accounting
            .record_latency(recv_ts - send_ts);
        node_ref.operate(|e| {
            e.answer_rcvd(send_ts, recv_ts, bytes);
        })
    }
    pub fn stats_question_lost(&self, node_ref: NodeRef) {
        node_ref.operate(|e| {
            e.question_lost();
        })
    }
    pub fn stats_failed_to_send(&self, node_ref: NodeRef, ts: u64, expects_answer: bool) {
        node_ref.operate(|e| {
            e.failed_to_send(ts, expects_answer);
        })
    }

    //////////////////////////////////////////////////////////////////////
    // Routing Table Health Metrics

    pub fn get_routing_table_health(&self) -> RoutingTableHealth {
        let mut health = RoutingTableHealth::default();
        let cur_ts = intf::get_timestamp();
        let inner = self.inner.lock();
        for bucket in &inner.buckets {
            for entry in bucket.entries() {
                match entry.1.state(cur_ts) {
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
}
