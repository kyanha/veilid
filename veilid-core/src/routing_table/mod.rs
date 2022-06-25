mod bucket;
mod bucket_entry;
mod debug;
mod find_nodes;
mod node_ref;
mod stats_accounting;
mod tasks;

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
    rolling_transfers_task: TickTask,
    bootstrap_task: TickTask,
    peer_minimum_refresh_task: TickTask,
    ping_validator_task: TickTask,
    node_info_update_single_future: MustJoinSingleFuture<()>,
    kick_buckets_task: TickTask,
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
    fn new_unlocked_inner(config: VeilidConfig) -> RoutingTableUnlockedInner {
        let c = config.get();
        RoutingTableUnlockedInner {
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
            bootstrap_task: TickTask::new(1),
            peer_minimum_refresh_task: TickTask::new_ms(c.network.dht.min_peer_refresh_time_ms),
            ping_validator_task: TickTask::new(1),
            node_info_update_single_future: MustJoinSingleFuture::new(),
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
        // Set bootstrap tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .bootstrap_task
                .set_routine(move |s, _l, _t| Box::pin(this2.clone().bootstrap_task_routine(s)));
        }
        // Set peer minimum refresh tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .peer_minimum_refresh_task
                .set_routine(move |s, _l, _t| {
                    Box::pin(this2.clone().peer_minimum_refresh_task_routine(s))
                });
        }
        // Set ping validator tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .ping_validator_task
                .set_routine(move |s, l, t| {
                    Box::pin(this2.clone().ping_validator_task_routine(s, l, t))
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
            let cur_ts = intf::get_timestamp();
            Self::with_entries(&*inner, cur_ts, BucketEntryState::Dead, |_, v| {
                v.with_mut(|e| e.set_seen_our_node_info(false));
                Option::<()>::None
            });
        }

        Ok(())
    }

    pub fn clear_dial_info_details(&self, domain: RoutingDomain) {
        trace!("clearing dial info domain: {:?}", domain);

        let mut inner = self.inner.write();
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
        debug!("stopping bootstrap task");
        if let Err(e) = self.unlocked_inner.bootstrap_task.stop().await {
            error!("bootstrap_task not stopped: {}", e);
        }
        debug!("stopping peer minimum refresh task");
        if let Err(e) = self.unlocked_inner.peer_minimum_refresh_task.stop().await {
            error!("peer_minimum_refresh_task not stopped: {}", e);
        }
        debug!("stopping ping_validator task");
        if let Err(e) = self.unlocked_inner.ping_validator_task.stop().await {
            error!("ping_validator_task not stopped: {}", e);
        }
        debug!("stopping node info update singlefuture");
        if self
            .unlocked_inner
            .node_info_update_single_future
            .join()
            .await
            .is_err()
        {
            error!("node_info_update_single_future not stopped");
        }

        *self.inner.write() = Self::new_inner(self.network_manager());

        debug!("finished routing table terminate");
    }

    // Inform routing table entries that our dial info has changed
    pub async fn send_node_info_updates(&self) {
        let this = self.clone();
        // Run in background only once
        let _ = self
            .clone()
            .unlocked_inner
            .node_info_update_single_future
            .single_spawn(async move {
                // Only update if we actually have a valid network class
                let netman = this.network_manager();
                if matches!(
                    netman.get_network_class().unwrap_or(NetworkClass::Invalid),
                    NetworkClass::Invalid
                ) {
                    trace!(
                        "not sending node info update because our network class is not yet valid"
                    );
                    return;
                }

                // Get the list of refs to all nodes to update
                let node_refs = {
                    let inner = this.inner.read();
                    let mut node_refs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
                    let cur_ts = intf::get_timestamp();
                    Self::with_entries(&*inner, cur_ts, BucketEntryState::Unreliable, |k, v| {
                        // Only update nodes that haven't seen our node info yet
                        if !v.with(|e| e.has_seen_our_node_info()) {
                            node_refs.push(NodeRef::new(this.clone(), k, v, None));
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
            .await;
    }

    // Attempt to empty the routing table
    // should only be performed when there are no node_refs (detached)
    pub fn purge(&self) {
        let mut inner = self.inner.write();
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

    fn get_entry_count(inner: &RoutingTableInner, min_state: BucketEntryState) -> usize {
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

    fn queue_bucket_kick(&self, node_id: DHTKey) {
        let mut inner = self.inner.write();
        let idx = Self::find_bucket_index(&*inner, node_id);
        inner.kick_queue.insert(idx);
    }

    // Create a node reference, possibly creating a bucket entry
    // the 'update_func' closure is called on the node, and, if created,
    // in a locked fashion as to ensure the bucket entry state is always valid
    pub fn create_node_ref<F>(&self, node_id: DHTKey, update_func: F) -> Result<NodeRef, String>
    where
        F: FnOnce(&mut BucketEntryInner),
    {
        // Ensure someone isn't trying register this node itself
        if node_id == self.node_id() {
            return Err("can't register own node".to_owned()).map_err(logthru_rtab!(error));
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
                log_rtab!(debug "Routing table now has {} nodes, {} live", cnt, Self::get_entry_count(&mut *inner, BucketEntryState::Unreliable));
                let bucket = &mut inner.buckets[idx];
                let nr = bucket.add_entry(node_id);

                // Update the entry
                let entry = bucket.entry(&node_id).unwrap();
                entry.with_mut(update_func);

                // Kick the bucket
                // It is important to do this in the same inner lock as the add_entry
                inner.kick_queue.insert(idx);

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

        Ok(noderef)
    }

    pub fn lookup_node_ref(&self, node_id: DHTKey) -> Option<NodeRef> {
        let inner = self.inner.read();
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

    // fn operate_on_bucket_entry_inner_locked<T, F>(
    //     inner: &RoutingTableInner,
    //     node_id: DHTKey,
    //     f: F,
    // ) -> T
    // where
    //     F: FnOnce(&BucketEntryInner) -> T,
    // {
    //     let idx = Self::find_bucket_index(&*inner, node_id);
    //     let bucket = &inner.buckets[idx];
    //     let entry = bucket.entry(&node_id).unwrap();
    //     entry.with(f)
    // }

    // fn operate_on_bucket_entry_inner_locked_mut<T, F>(
    //     inner: &RoutingTableInner,
    //     node_id: DHTKey,
    //     f: F,
    // ) -> T
    // where
    //     F: FnOnce(&mut BucketEntryInner) -> T,
    // {
    //     let idx = Self::find_bucket_index(&*inner, node_id);
    //     let bucket = &inner.buckets[idx];
    //     let entry = bucket.entry(&node_id).unwrap();
    //     entry.with_mut(f)
    // }

    // fn operate_on_bucket_entry<T, F>(&self, node_id: DHTKey, f: F) -> T
    // where
    //     F: FnOnce(&BucketEntryInner) -> T,
    // {
    //     let inner = self.inner.read();
    //     Self::operate_on_bucket_entry_inner_locked(&mut *inner, node_id, f)
    // }

    // fn operate_on_bucket_entry_mut<T, F>(&self, node_id: DHTKey, f: F) -> T
    // where
    //     F: FnOnce(&mut BucketEntryInner) -> T,
    // {
    //     let inner = self.inner.read();
    //     Self::operate_on_bucket_entry_inner_locked_mut(&*inner, node_id, f)
    // }

    // Ticks about once per second
    // to run tick tasks which may run at slower tick rates as configured
    pub async fn tick(&self) -> Result<(), String> {
        // Do rolling transfers every ROLLING_TRANSFERS_INTERVAL_SECS secs
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // If routing table has no live entries, then add the bootstrap nodes to it
        let live_entry_count =
            Self::get_entry_count(&*self.inner.read(), BucketEntryState::Unreliable);

        if live_entry_count == 0 {
            self.unlocked_inner.bootstrap_task.tick().await?;
        }

        // If we still don't have enough peers, find nodes until we do
        let min_peer_count = {
            let c = self.config.get();
            c.network.dht.min_peer_count as usize
        };
        if live_entry_count < min_peer_count {
            self.unlocked_inner.peer_minimum_refresh_task.tick().await?;
        }

        // Ping validate some nodes to groom the table
        self.unlocked_inner.ping_validator_task.tick().await?;

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
