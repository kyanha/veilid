mod bucket;
mod bucket_entry;
mod debug;
mod find_nodes;
mod node_ref;
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
pub use node_ref::*;
pub use stats_accounting::*;

//////////////////////////////////////////////////////////////////////////

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
    node_id: DHTKey,              // The current node's public DHT key
    node_id_secret: DHTKeySecret, // The current node's DHT key secret

    buckets: Vec<Bucket>,        // Routing table buckets that hold entries
    kick_queue: BTreeSet<usize>, // Buckets to kick on our next kick task
    bucket_entry_count: usize,   // A fast counter for the number of entries in the table, total

    public_internet_routing_domain: RoutingDomainDetail, // The dial info we use on the public internet
    local_network_routing_domain: RoutingDomainDetail, // The dial info we use on the local network

    self_latency_stats_accounting: LatencyStatsAccounting, // Interim accounting mechanism for this node's RPC latency to any other node
    self_transfer_stats_accounting: TransferStatsAccounting, // Interim accounting mechanism for the total bandwidth to/from this node
    self_transfer_stats: TransferStatsDownUp, // Statistics about the total bandwidth to/from this node
}

#[derive(Clone, Debug, Default)]
pub struct RoutingTableHealth {
    pub reliable_entry_count: usize,
    pub unreliable_entry_count: usize,
    pub dead_entry_count: usize,
}

struct RoutingTableUnlockedInner {
    // Background processes
    rolling_transfers_task: TickTask<EyreReport>,
    kick_buckets_task: TickTask<EyreReport>,
}

#[derive(Clone)]
pub struct RoutingTable {
    config: VeilidConfig,
    inner: Arc<RwLock<RoutingTableInner>>,
    unlocked_inner: Arc<RoutingTableUnlockedInner>,
}

impl RoutingTable {
    fn new_inner(network_manager: NetworkManager) -> RoutingTableInner {
        RoutingTableInner {
            network_manager,
            node_id: DHTKey::default(),
            node_id_secret: DHTKeySecret::default(),
            buckets: Vec::new(),
            kick_queue: BTreeSet::default(),
            public_internet_routing_domain: RoutingDomainDetail::default(),
            local_network_routing_domain: RoutingDomainDetail::default(),
            bucket_entry_count: 0,
            self_latency_stats_accounting: LatencyStatsAccounting::new(),
            self_transfer_stats_accounting: TransferStatsAccounting::new(),
            self_transfer_stats: TransferStatsDownUp::default(),
        }
    }
    fn new_unlocked_inner(_config: VeilidConfig) -> RoutingTableUnlockedInner {
        //let c = config.get();
        RoutingTableUnlockedInner {
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
            kick_buckets_task: TickTask::new(1),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        let config = network_manager.config();
        let this = Self {
            config: config.clone(),
            inner: Arc::new(RwLock::new(Self::new_inner(network_manager))),
            unlocked_inner: Arc::new(Self::new_unlocked_inner(config)),
        };
        // Set rolling transfers tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .rolling_transfers_task
                .set_routine(move |s, l, t| {
                    Box::pin(this2.clone().rolling_transfers_task_routine(s, l, t))
                });
        }

        // Set kick buckets tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .kick_buckets_task
                .set_routine(move |s, l, t| {
                    Box::pin(this2.clone().kick_buckets_task_routine(s, l, t))
                });
        }
        this
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.read().network_manager.clone()
    }
    pub fn rpc_processor(&self) -> RPCProcessor {
        self.network_manager().rpc_processor()
    }

    pub fn node_id(&self) -> DHTKey {
        self.inner.read().node_id
    }

    pub fn node_id_secret(&self) -> DHTKeySecret {
        self.inner.read().node_id_secret
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
        let inner = self.inner.read();
        Self::with_routing_domain(&*inner, domain, |rd| !rd.dial_info_details.is_empty())
    }

    pub fn dial_info_details(&self, domain: RoutingDomain) -> Vec<DialInfoDetail> {
        let inner = self.inner.read();
        Self::with_routing_domain(&*inner, domain, |rd| rd.dial_info_details.clone())
    }

    pub fn first_filtered_dial_info_detail(
        &self,
        domain: Option<RoutingDomain>,
        filter: &DialInfoFilter,
    ) -> Option<DialInfoDetail> {
        let inner = self.inner.read();
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
        let inner = self.inner.read();
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
    ) -> EyreResult<()> {
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
            bail!("shouldn't be registering local addresses as public");
        }
        if !dial_info.is_valid() {
            bail!(
                "shouldn't be registering invalid addresses: {:?}",
                dial_info
            );
        }

        let mut inner = self.inner.write();
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
            Self::reset_all_seen_our_node_info(&*inner);
        }

        Ok(())
    }

    fn reset_all_seen_our_node_info(inner: &RoutingTableInner) {
        let cur_ts = intf::get_timestamp();
        Self::with_entries(&*inner, cur_ts, BucketEntryState::Dead, |_, v| {
            v.with_mut(|e| e.set_seen_our_node_info(false));
            Option::<()>::None
        });
    }

    pub fn clear_dial_info_details(&self, domain: RoutingDomain) {
        trace!("clearing dial info domain: {:?}", domain);

        let mut inner = self.inner.write();
        Self::with_routing_domain_mut(&mut *inner, domain, |rd| {
            rd.dial_info_details.clear();
        });

        // Public dial info changed, go through all nodes and reset their 'seen our node info' bit
        if matches!(domain, RoutingDomain::PublicInternet) {
            Self::reset_all_seen_our_node_info(&*inner);
        }
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

        // make local copy of node id for easy access
        let c = self.config.get();
        inner.node_id = c.network.node_id;
        inner.node_id_secret = c.network.node_id_secret;

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

        *self.inner.write() = Self::new_inner(self.network_manager());

        debug!("finished routing table terminate");
    }

    // Attempt to empty the routing table
    // should only be performed when there are no node_refs (detached)
    pub fn purge_buckets(&self) {
        let mut inner = self.inner.write();
        log_rtab!(
            "Starting routing table buckets purge. Table currently has {} nodes",
            inner.bucket_entry_count
        );
        for bucket in &mut inner.buckets {
            bucket.kick(0);
        }
        log_rtab!(debug
             "Routing table buckets purge complete. Routing table now has {} nodes",
            inner.bucket_entry_count
        );
    }

    // Attempt to remove last_connections from entries
    pub fn purge_last_connections(&self) {
        let mut inner = self.inner.write();
        log_rtab!(
            "Starting routing table last_connections purge. Table currently has {} nodes",
            inner.bucket_entry_count
        );
        for bucket in &mut inner.buckets {
            for entry in bucket.entries() {
                entry.1.with_mut(|e| {
                    e.clear_last_connection();
                });
            }
        }
        log_rtab!(debug
             "Routing table last_connections purge complete. Routing table now has {} nodes",
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

    pub fn get_entry_count(&self, min_state: BucketEntryState) -> usize {
        let inner = self.inner.read();
        Self::get_entry_count_inner(&*inner, min_state)
    }

    fn get_entry_count_inner(inner: &RoutingTableInner, min_state: BucketEntryState) -> usize {
        let mut count = 0usize;
        let cur_ts = intf::get_timestamp();
        Self::with_entries(inner, cur_ts, min_state, |_, _| {
            count += 1;
            Option::<()>::None
        });
        count
    }

    fn with_entries<T, F: FnMut(DHTKey, Arc<BucketEntry>) -> Option<T>>(
        inner: &RoutingTableInner,
        cur_ts: u64,
        min_state: BucketEntryState,
        mut f: F,
    ) -> Option<T> {
        for bucket in &inner.buckets {
            for entry in bucket.entries() {
                if entry.1.with(|e| e.state(cur_ts) >= min_state) {
                    if let Some(out) = f(*entry.0, entry.1.clone()) {
                        return Some(out);
                    }
                }
            }
        }
        None
    }

    pub fn get_nodes_needing_updates(&self, cur_ts: u64, all: bool) -> Vec<NodeRef> {
        let inner = self.inner.read();
        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
        Self::with_entries(&*inner, cur_ts, BucketEntryState::Unreliable, |k, v| {
            // Only update nodes that haven't seen our node info yet
            if all || !v.with(|e| e.has_seen_our_node_info()) {
                node_refs.push(NodeRef::new(self.clone(), k, v, None));
            }
            Option::<()>::None
        });
        node_refs
    }

    pub fn get_nodes_needing_ping(
        &self,
        cur_ts: u64,
        relay_node_id: Option<DHTKey>,
    ) -> Vec<NodeRef> {
        let inner = self.inner.read();
        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
        Self::with_entries(&*inner, cur_ts, BucketEntryState::Unreliable, |k, v| {
            // Only update nodes that haven't seen our node info yet
            if v.with(|e| e.needs_ping(&k, cur_ts, relay_node_id)) {
                node_refs.push(NodeRef::new(self.clone(), k, v, None));
            }
            Option::<()>::None
        });
        node_refs
    }

    pub fn get_all_nodes(&self, cur_ts: u64) -> Vec<NodeRef> {
        let inner = self.inner.read();
        let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
        Self::with_entries(&*inner, cur_ts, BucketEntryState::Unreliable, |k, v| {
            node_refs.push(NodeRef::new(self.clone(), k, v, None));
            Option::<()>::None
        });
        node_refs
    }

    fn queue_bucket_kick(&self, node_id: DHTKey) {
        let mut inner = self.inner.write();
        let idx = Self::find_bucket_index(&*inner, node_id);
        inner.kick_queue.insert(idx);
    }

    // Create a node reference, possibly creating a bucket entry
    // the 'update_func' closure is called on the node, and, if created,
    // in a locked fashion as to ensure the bucket entry state is always valid
    pub fn create_node_ref<F>(&self, node_id: DHTKey, update_func: F) -> Option<NodeRef>
    where
        F: FnOnce(&mut BucketEntryInner),
    {
        // Ensure someone isn't trying register this node itself
        if node_id == self.node_id() {
            log_rtab!(debug "can't register own node");
            return None;
        }

        // Lock this entire operation
        let mut inner = self.inner.write();

        // Look up existing entry
        let idx = Self::find_bucket_index(&*inner, node_id);
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
                entry.with_mut(update_func);

                // Kick the bucket
                inner.kick_queue.insert(idx);
                log_rtab!(debug "Routing table now has {} nodes, {} live", cnt, Self::get_entry_count_inner(&mut *inner, BucketEntryState::Unreliable));

                nr
            }
            Some(nr) => {
                // Update the entry
                let bucket = &mut inner.buckets[idx];
                let entry = bucket.entry(&node_id).unwrap();
                entry.with_mut(|e| {
                    update_func(e);
                });

                nr
            }
        };

        Some(noderef)
    }

    pub fn lookup_node_ref(&self, node_id: DHTKey) -> Option<NodeRef> {
        let inner = self.inner.read();
        if node_id == inner.node_id {
            log_rtab!(debug "can't look up own node id in routing table");
            return None;
        }
        let idx = Self::find_bucket_index(&*inner, node_id);
        let bucket = &inner.buckets[idx];
        bucket
            .entry(&node_id)
            .map(|e| NodeRef::new(self.clone(), node_id, e, None))
    }

    // Shortcut function to add a node to our routing table if it doesn't exist
    // and add the dial info we have for it, since that's pretty common
    pub fn register_node_with_signed_node_info(
        &self,
        node_id: DHTKey,
        signed_node_info: SignedNodeInfo,
    ) -> Option<NodeRef> {
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

        self.create_node_ref(node_id, |e| {
            if e.update_node_info(signed_node_info) {
                // at least someone thought this node was live and its node info changed so lets try to contact it
                e.touch_last_seen(intf::get_timestamp());
            }
        })
    }

    // Shortcut function to add a node to our routing table if it doesn't exist
    // and add the last peer address we have for it, since that's pretty common
    pub fn register_node_with_existing_connection(
        &self,
        node_id: DHTKey,
        descriptor: ConnectionDescriptor,
        timestamp: u64,
    ) -> Option<NodeRef> {
        self.create_node_ref(node_id, |e| {
            // set the most recent node address for connection finding and udp replies
            e.set_last_connection(descriptor, timestamp);

            // this node is live because it literally just connected to us
            e.touch_last_seen(timestamp);
        })
    }

    // Ticks about once per second
    // to run tick tasks which may run at slower tick rates as configured
    pub async fn tick(&self) -> EyreResult<()> {
        // Do rolling transfers every ROLLING_TRANSFERS_INTERVAL_SECS secs
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // Kick buckets task
        let kick_bucket_queue_count = { self.inner.read().kick_queue.len() };
        if kick_bucket_queue_count > 0 {
            self.unlocked_inner.kick_buckets_task.tick().await?;
        }

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
            .write()
            .self_transfer_stats_accounting
            .add_up(bytes);
        node_ref.operate_mut(|e| {
            e.question_sent(ts, bytes, expects_answer);
        })
    }
    pub fn stats_question_rcvd(&self, node_ref: NodeRef, ts: u64, bytes: u64) {
        self.inner
            .write()
            .self_transfer_stats_accounting
            .add_down(bytes);
        node_ref.operate_mut(|e| {
            e.question_rcvd(ts, bytes);
        })
    }
    pub fn stats_answer_sent(&self, node_ref: NodeRef, bytes: u64) {
        self.inner
            .write()
            .self_transfer_stats_accounting
            .add_up(bytes);
        node_ref.operate_mut(|e| {
            e.answer_sent(bytes);
        })
    }
    pub fn stats_answer_rcvd(&self, node_ref: NodeRef, send_ts: u64, recv_ts: u64, bytes: u64) {
        {
            let mut inner = self.inner.write();
            inner.self_transfer_stats_accounting.add_down(bytes);
            inner
                .self_latency_stats_accounting
                .record_latency(recv_ts - send_ts);
        }
        node_ref.operate_mut(|e| {
            e.answer_rcvd(send_ts, recv_ts, bytes);
        })
    }
    pub fn stats_question_lost(&self, node_ref: NodeRef) {
        node_ref.operate_mut(|e| {
            e.question_lost();
        })
    }
    pub fn stats_failed_to_send(&self, node_ref: NodeRef, ts: u64, expects_answer: bool) {
        node_ref.operate_mut(|e| {
            e.failed_to_send(ts, expects_answer);
        })
    }

    //////////////////////////////////////////////////////////////////////
    // Routing Table Health Metrics

    pub fn get_routing_table_health(&self) -> RoutingTableHealth {
        let mut health = RoutingTableHealth::default();
        let cur_ts = intf::get_timestamp();
        let inner = self.inner.read();
        for bucket in &inner.buckets {
            for (_, v) in bucket.entries() {
                match v.with(|e| e.state(cur_ts)) {
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
