use super::*;
use crate::dht::*;
use alloc::fmt;

pub struct NodeRef {
    routing_table: RoutingTable,
    node_id: DHTKey,
    dial_info_filter: DialInfoFilter,
}

impl NodeRef {
    pub fn new(routing_table: RoutingTable, key: DHTKey, entry: &mut BucketEntry) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
            dial_info_filter: DialInfoFilter::default(),
        }
    }
    pub fn new_filtered(
        routing_table: RoutingTable,
        key: DHTKey,
        entry: &mut BucketEntry,
        dial_info_filter: DialInfoFilter,
    ) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
            dial_info_filter,
        }
    }

    pub fn node_id(&self) -> DHTKey {
        self.node_id
    }

    pub fn dial_info_filter(&self) -> &DialInfoFilter {
        &self.dial_info_filter
    }

    pub fn operate<T, F>(&self, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        self.routing_table.operate_on_bucket_entry(self.node_id, f)
    }

    pub fn node_info(&self) -> NodeInfo {
        self.operate(|e| e.node_info().clone())
    }

    pub fn has_dial_info(&self) -> bool {
        self.operate(|e| !e.node_info().dial_infos.is_empty())
    }

    // Returns if this node has seen and acknowledged our node's dial info yet
    pub fn has_seen_our_dial_info(&self) -> bool {
        self.operate(|e| e.has_seen_our_dial_info())
    }
    pub fn set_seen_our_dial_info(&self) {
        self.operate(|e| e.set_seen_our_dial_info(true));
    }

    // Returns the best node info to attempt a connection to this node
    pub fn best_node_info(&self) -> Option<NodeInfo> {
        let nm = self.routing_table.network_manager();
        let protocol_config = nm.get_protocol_config()?;
        self.operate(|e| {
            e.first_filtered_node_info(|di| {
                // Does it match the dial info filter
                if !di.matches_filter(&self.dial_info_filter) {
                    return false;
                }
                // Filter out dial infos that don't match our protocol config
                // for outbound connections. This routine filters on 'connect' settings
                // to ensure we connect using only the protocols we have enabled.
                protocol_config.is_protocol_type_connect_enabled(di.protocol_type())
            })
        })
    }
    pub fn last_connection(&self) -> Option<ConnectionDescriptor> {
        match self.operate(|e| e.last_connection()) {
            None => None,
            Some(c) => {
                if !c.matches_filter(&self.dial_info_filter) {
                    return None;
                }
                // We don't filter this out by protocol config because if a connection
                // succeeded, it's allowed to persist and be used for communication
                // regardless of any other configuration
                Some(c)
            }
        }
    }
}

impl Clone for NodeRef {
    fn clone(&self) -> Self {
        self.operate(move |e| {
            e.ref_count += 1;
        });
        Self {
            routing_table: self.routing_table.clone(),
            node_id: self.node_id,
            dial_info_filter: self.dial_info_filter.clone(),
        }
    }
}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = self.node_id.encode();
        if !self.dial_info_filter.is_empty() {
            out += &format!("{:?}", self.dial_info_filter);
        }
        write!(f, "{}", out)
    }
}

impl Drop for NodeRef {
    fn drop(&mut self) {
        self.routing_table.drop_node_ref(self.node_id);
    }
}
