use super::*;

use crate::dht::*;
use crate::intf::*;
use crate::xx::*;
use crate::*;

pub type FilterType = Box<dyn Fn(&(&DHTKey, Option<&mut BucketEntry>)) -> bool>;

impl RoutingTable {
    // Retrieve the fastest nodes in the routing table with a particular kind of protocol and address type
    // Returns noderefs are are scoped to that address type only
    pub fn find_fast_public_nodes_filtered(
        &self,
        dial_info_filter: &DialInfoFilter,
    ) -> Vec<NodeRef> {
        let dial_info_filter1 = dial_info_filter.clone();
        self.find_fastest_nodes(
            // filter
            Some(Box::new(
                move |params: &(&DHTKey, Option<&mut BucketEntry>)| {
                    let entry = params.1.as_ref().unwrap();

                    // skip nodes on our local network here
                    if entry.local_node_info().has_dial_info() {
                        return false;
                    }

                    // does it have matching public dial info?
                    entry
                        .node_info()
                        .first_filtered_dial_info_detail(|did| {
                            did.matches_filter(&dial_info_filter1)
                        })
                        .is_some()
                },
            )),
            // transform
            |e| {
                NodeRef::new(
                    self.clone(),
                    *e.0,
                    e.1.as_mut().unwrap(),
                    Some(dial_info_filter.clone()),
                )
            },
        )
    }

    // Get our own node's peer info (public node info) so we can share it with other nodes
    pub fn get_own_peer_info(&self) -> PeerInfo {
        let netman = self.network_manager();
        let relay_node = netman.relay_node();
        PeerInfo {
            node_id: NodeId::new(self.node_id()),
            node_info: NodeInfo {
                network_class: netman.get_network_class().unwrap_or(NetworkClass::Invalid),
                outbound_protocols: netman.get_protocol_config().unwrap_or_default().outbound,
                dial_info_detail_list: self.dial_info_details(RoutingDomain::PublicInternet),
                relay_peer_info: relay_node.map(|rn| Box::new(rn.peer_info())),
            },
        }
    }

    pub fn transform_to_peer_info(
        kv: &mut (&DHTKey, Option<&mut BucketEntry>),
        own_peer_info: &PeerInfo,
    ) -> PeerInfo {
        match &kv.1 {
            None => own_peer_info.clone(),
            Some(entry) => entry.peer_info(*kv.0),
        }
    }

    pub fn find_peers_with_sort_and_filter<F, C, T, O>(
        &self,
        node_count: usize,
        cur_ts: u64,
        filter: F,
        compare: C,
        transform: T,
    ) -> Vec<O>
    where
        F: Fn(&(&DHTKey, Option<&mut BucketEntry>)) -> bool,
        C: Fn(
            &(&DHTKey, Option<&mut BucketEntry>),
            &(&DHTKey, Option<&mut BucketEntry>),
        ) -> core::cmp::Ordering,
        T: Fn(&mut (&DHTKey, Option<&mut BucketEntry>)) -> O,
    {
        let mut inner = self.inner.lock();

        // collect all the nodes for sorting
        let mut nodes =
            Vec::<(&DHTKey, Option<&mut BucketEntry>)>::with_capacity(inner.bucket_entry_count + 1);
        // add our own node (only one of there with the None entry)
        let self_node_id = inner.node_id;
        let selfkv = (&self_node_id, None);
        if filter(&selfkv) {
            nodes.push(selfkv);
        }
        // add all nodes from buckets
        for b in &mut inner.buckets {
            for (k, v) in b.entries_mut() {
                // Don't bother with dead nodes
                if !v.check_dead(cur_ts) {
                    // Apply filter
                    let kv = (k, Some(v));
                    if filter(&kv) {
                        nodes.push(kv);
                    }
                }
            }
        }

        // sort by preference for returning nodes
        nodes.sort_by(compare);

        // return transformed vector for filtered+sorted nodes
        let cnt = usize::min(node_count, nodes.len());
        let mut out = Vec::<O>::with_capacity(cnt);
        for mut node in nodes {
            let val = transform(&mut node);
            out.push(val);
        }

        out
    }

    pub fn find_fastest_nodes<T, O>(&self, filter: Option<FilterType>, transform: T) -> Vec<O>
    where
        T: Fn(&mut (&DHTKey, Option<&mut BucketEntry>)) -> O,
    {
        let cur_ts = get_timestamp();
        let node_count = {
            let c = self.config.get();
            c.network.dht.max_find_node_count as usize
        };
        let out = self.find_peers_with_sort_and_filter(
            node_count,
            cur_ts,
            // filter
            |kv| {
                if kv.1.is_none() {
                    // filter out self peer, as it is irrelevant to the 'fastest nodes' search
                    return false;
                }
                if filter.is_some() && !filter.as_ref().unwrap()(kv) {
                    return false;
                }
                true
            },
            // sort
            |(a_key, a_entry), (b_key, b_entry)| {
                // same nodes are always the same
                if a_key == b_key {
                    return core::cmp::Ordering::Equal;
                }
                // our own node always comes last (should not happen, here for completeness)
                if a_entry.is_none() {
                    return core::cmp::Ordering::Greater;
                }
                if b_entry.is_none() {
                    return core::cmp::Ordering::Less;
                }
                // reliable nodes come first
                let ae = a_entry.as_ref().unwrap();
                let be = b_entry.as_ref().unwrap();

                let ra = ae.check_reliable(cur_ts);
                let rb = be.check_reliable(cur_ts);
                if ra != rb {
                    if ra {
                        return core::cmp::Ordering::Less;
                    } else {
                        return core::cmp::Ordering::Greater;
                    }
                }

                // latency is the next metric, closer nodes first
                let a_latency = match ae.peer_stats().latency.as_ref() {
                    None => {
                        // treat unknown latency as slow
                        return core::cmp::Ordering::Greater;
                    }
                    Some(l) => l,
                };
                let b_latency = match be.peer_stats().latency.as_ref() {
                    None => {
                        // treat unknown latency as slow
                        return core::cmp::Ordering::Less;
                    }
                    Some(l) => l,
                };
                // Sort by average latency
                a_latency.average.cmp(&b_latency.average)
            },
            // transform,
            transform,
        );
        log_rtab!(">> find_fastest_nodes: node count = {}", out.len());
        out
    }

    pub fn find_closest_nodes<T, O>(
        &self,
        node_id: DHTKey,
        filter: Option<FilterType>,
        transform: T,
    ) -> Vec<O>
    where
        T: Fn(&mut (&DHTKey, Option<&mut BucketEntry>)) -> O,
    {
        let cur_ts = get_timestamp();
        let node_count = {
            let c = self.config.get();
            c.network.dht.max_find_node_count as usize
        };
        let out = self.find_peers_with_sort_and_filter(
            node_count,
            cur_ts,
            // filter
            |kv| {
                if kv.1.is_none() {
                    // include self peer, as it is relevant to the 'closest nodes' search
                    return true;
                }
                if filter.is_some() && !filter.as_ref().unwrap()(kv) {
                    return false;
                }
                true
            },
            // sort
            |(a_key, a_entry), (b_key, b_entry)| {
                // same nodes are always the same
                if a_key == b_key {
                    return core::cmp::Ordering::Equal;
                }
                // reliable nodes come first, pessimistically treating our own node as unreliable
                let ra = a_entry.as_ref().map_or(false, |x| x.check_reliable(cur_ts));
                let rb = b_entry.as_ref().map_or(false, |x| x.check_reliable(cur_ts));
                if ra != rb {
                    if ra {
                        return core::cmp::Ordering::Less;
                    } else {
                        return core::cmp::Ordering::Greater;
                    }
                }

                // distance is the next metric, closer nodes first
                let da = distance(a_key, &node_id);
                let db = distance(b_key, &node_id);
                da.cmp(&db)
            },
            // transform,
            transform,
        );
        log_rtab!(">> find_closest_nodes: node count = {}", out.len());
        out
    }
}
