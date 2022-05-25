use super::*;
use crate::dht::*;
use alloc::fmt;

// Connectionless protocols like UDP are dependent on a NAT translation timeout
// We should ping them with some frequency and 30 seconds is typical timeout
const CONNECTIONLESS_TIMEOUT_SECS: u32 = 29;

pub struct NodeRef {
    routing_table: RoutingTable,
    node_id: DHTKey,
    filter: Option<DialInfoFilter>,
    #[cfg(feature = "tracking")]
    track_id: usize,
}

impl NodeRef {
    pub fn new(
        routing_table: RoutingTable,
        key: DHTKey,
        entry: &mut BucketEntry,
        filter: Option<DialInfoFilter>,
    ) -> Self {
        entry.ref_count += 1;

        Self {
            routing_table,
            node_id: key,
            filter,
            #[cfg(feature = "tracking")]
            track_id: entry.track(),
        }
    }

    pub fn node_id(&self) -> DHTKey {
        self.node_id
    }

    pub fn filter_ref(&self) -> Option<&DialInfoFilter> {
        self.filter.as_ref()
    }

    pub fn take_filter(&mut self) -> Option<DialInfoFilter> {
        self.filter.take()
    }

    pub fn set_filter(&mut self, filter: Option<DialInfoFilter>) {
        self.filter = filter
    }

    // Returns true if some protocols can still pass the filter and false if no protocols remain
    pub fn filter_protocols(&mut self, protocol_set: ProtocolSet) -> bool {
        if protocol_set != ProtocolSet::all() {
            let mut dif = self.filter.clone().unwrap_or_default();
            dif.protocol_set &= protocol_set;
            self.filter = Some(dif);
        }
        self.filter
            .as_ref()
            .map(|f| !f.protocol_set.is_empty())
            .unwrap_or(true)
    }

    pub fn operate<T, F>(&self, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        self.routing_table.operate_on_bucket_entry(self.node_id, f)
    }

    pub fn peer_info(&self) -> Option<PeerInfo> {
        self.operate(|e| e.peer_info(self.node_id()))
    }
    pub fn has_seen_our_node_info(&self) -> bool {
        self.operate(|e| e.has_seen_our_node_info())
    }
    pub fn set_seen_our_node_info(&self) {
        self.operate(|e| e.set_seen_our_node_info(true));
    }
    pub fn network_class(&self) -> Option<NetworkClass> {
        self.operate(|e| e.node_info().map(|n| n.network_class))
    }
    pub fn outbound_protocols(&self) -> Option<ProtocolSet> {
        self.operate(|e| e.node_info().map(|n| n.outbound_protocols))
    }
    pub fn relay(&self) -> Option<NodeRef> {
        let target_rpi = self.operate(|e| e.node_info().map(|n| n.relay_peer_info))?;
        target_rpi.and_then(|t| {
            // If relay is ourselves, then return None, because we can't relay through ourselves
            // and to contact this node we should have had an existing inbound connection
            if t.node_id.key == self.routing_table.node_id() {
                return None;
            }

            // Register relay node and return noderef
            self.routing_table
                .register_node_with_signed_node_info(t.node_id.key, t.signed_node_info)
                .map_err(logthru_rtab!(error))
                .ok()
                .map(|mut nr| {
                    nr.set_filter(self.filter_ref().cloned());
                    nr
                })
        })
    }
    pub fn first_filtered_dial_info_detail(
        &self,
        routing_domain: Option<RoutingDomain>,
    ) -> Option<DialInfoDetail> {
        self.operate(|e| {
            // Prefer local dial info first unless it is filtered out
            if (routing_domain == None || routing_domain == Some(RoutingDomain::LocalNetwork))
                && matches!(
                    self.filter
                        .as_ref()
                        .map(|f| f.peer_scope)
                        .unwrap_or(PeerScope::All),
                    PeerScope::All | PeerScope::Local
                )
            {
                e.local_node_info().and_then(|l| {
                    l.first_filtered_dial_info(|di| {
                        if let Some(filter) = self.filter.as_ref() {
                            di.matches_filter(filter)
                        } else {
                            true
                        }
                    })
                    .map(|di| DialInfoDetail {
                        class: DialInfoClass::Direct,
                        dial_info: di,
                    })
                })
            } else {
                None
            }
            .or_else(|| {
                if (routing_domain == None || routing_domain == Some(RoutingDomain::PublicInternet))
                    && matches!(
                        self.filter
                            .as_ref()
                            .map(|f| f.peer_scope)
                            .unwrap_or(PeerScope::All),
                        PeerScope::All | PeerScope::Global
                    )
                {
                    e.node_info().and_then(|n| {
                        n.first_filtered_dial_info_detail(|did| {
                            if let Some(filter) = self.filter.as_ref() {
                                did.matches_filter(filter)
                            } else {
                                true
                            }
                        })
                    })
                } else {
                    None
                }
            })
        })
    }

    pub fn all_filtered_dial_info_details<F>(
        &self,
        routing_domain: Option<RoutingDomain>,
    ) -> Vec<DialInfoDetail> {
        let mut out = Vec::new();
        self.operate(|e| {
            // Prefer local dial info first unless it is filtered out
            if (routing_domain == None || routing_domain == Some(RoutingDomain::LocalNetwork))
                && matches!(
                    self.filter
                        .as_ref()
                        .map(|f| f.peer_scope)
                        .unwrap_or(PeerScope::All),
                    PeerScope::All | PeerScope::Local
                )
            {
                if let Some(lni) = e.local_node_info() {
                    for di in lni.all_filtered_dial_info(|di| {
                        if let Some(filter) = self.filter.as_ref() {
                            di.matches_filter(filter)
                        } else {
                            true
                        }
                    }) {
                        out.push(DialInfoDetail {
                            class: DialInfoClass::Direct,
                            dial_info: di,
                        });
                    }
                }
            }
            if (routing_domain == None || routing_domain == Some(RoutingDomain::PublicInternet))
                && matches!(
                    self.filter
                        .as_ref()
                        .map(|f| f.peer_scope)
                        .unwrap_or(PeerScope::All),
                    PeerScope::All | PeerScope::Global
                )
            {
                if let Some(ni) = e.node_info() {
                    out.append(&mut ni.all_filtered_dial_info_details(|did| {
                        if let Some(filter) = self.filter.as_ref() {
                            did.matches_filter(filter)
                        } else {
                            true
                        }
                    }))
                }
            }
        });
        out.remove_duplicates();
        out
    }

    pub async fn last_connection(&self) -> Option<ConnectionDescriptor> {
        // Get the last connection and the last time we saw anything with this connection
        let (last_connection, last_seen) = self.operate(|e| {
            if let Some((last_connection, connection_ts)) = e.last_connection() {
                if let Some(last_seen_ts) = e.peer_stats().rpc_stats.last_seen_ts {
                    Some((last_connection, u64::max(last_seen_ts, connection_ts)))
                } else {
                    Some((last_connection, connection_ts))
                }
            } else {
                None
            }
        })?;
        // Should we check the connection table?
        if last_connection.protocol_type().is_connection_oriented() {
            // Look the connection up in the connection manager and see if it's still there
            let connection_manager = self.routing_table.network_manager().connection_manager();
            connection_manager.get_connection(last_connection).await?;
        } else {
            // If this is not connection oriented, then we check our last seen time
            // to see if this mapping has expired (beyond our timeout)
            let cur_ts = intf::get_timestamp();
            if (last_seen + (CONNECTIONLESS_TIMEOUT_SECS as u64 * 1_000_000u64)) < cur_ts {
                return None;
            }
        }
        Some(last_connection)
    }

    pub fn has_any_dial_info(&self) -> bool {
        self.operate(|e| {
            e.node_info()
                .map(|n| n.has_any_dial_info())
                .unwrap_or(false)
                || e.local_node_info()
                    .map(|l| l.has_dial_info())
                    .unwrap_or(false)
        })
    }
}

impl Clone for NodeRef {
    fn clone(&self) -> Self {
        self.operate(move |e| {
            e.ref_count += 1;

            Self {
                routing_table: self.routing_table.clone(),
                node_id: self.node_id,
                filter: self.filter.clone(),
                #[cfg(feature = "tracking")]
                track_id: e.track(),
            }
        })
    }
}

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl Eq for NodeRef {}

impl fmt::Display for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node_id.encode())
    }
}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRef")
            .field("node_id", &self.node_id)
            .field("filter", &self.filter)
            .finish()
    }
}

impl Drop for NodeRef {
    fn drop(&mut self) {
        #[cfg(feature = "tracking")]
        self.operate(|e| e.untrack(self.track_id));
        self.routing_table.drop_node_ref(self.node_id);
    }
}
