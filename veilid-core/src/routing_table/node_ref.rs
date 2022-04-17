use super::*;
use crate::dht::*;
use alloc::fmt;

pub struct NodeRef {
    routing_table: RoutingTable,
    node_id: DHTKey,
}

impl NodeRef {
    pub fn new(routing_table: RoutingTable, key: DHTKey, entry: &mut BucketEntry) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
        }
    }

    pub fn node_id(&self) -> DHTKey {
        self.node_id
    }

    pub fn operate<T, F>(&self, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        self.routing_table.operate_on_bucket_entry(self.node_id, f)
    }

    pub fn peer_info(&self) -> PeerInfo {
        self.operate(|e| e.peer_info(self.node_id()))
    }
    pub fn node_info(&self) -> NodeInfo {
        self.operate(|e| e.node_info().clone())
    }
    pub fn local_node_info(&self) -> LocalNodeInfo {
        self.operate(|e| e.local_node_info().clone())
    }
    pub fn has_seen_our_node_info(&self) -> bool {
        self.operate(|e| e.has_seen_our_node_info())
    }
    pub fn set_seen_our_node_info(&self) {
        self.operate(|e| e.set_seen_our_node_info(true));
    }
    pub fn last_connection(&self) -> Option<ConnectionDescriptor> {
        self.operate(|e| e.last_connection())
    }
    pub fn has_any_dial_info(&self) -> bool {
        self.operate(|e| e.node_info().has_any_dial_info() || e.local_node_info().has_dial_info())
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
        }
    }
}

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl Eq for NodeRef {}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node_id.encode())
    }
}

impl Drop for NodeRef {
    fn drop(&mut self) {
        self.routing_table.drop_node_ref(self.node_id);
    }
}
