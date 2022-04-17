use super::*;
use crate::dht::*;
use alloc::fmt;

// Connectionless protocols like UDP are dependent on a NAT translation timeout
// We should ping them with some frequency and 30 seconds is typical timeout
const CONNECTIONLESS_TIMEOUT_SECS: u32 = 29;

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
    pub async fn last_connection(&self) -> Option<ConnectionDescriptor> {
        // Get the last connection and the last time we saw anything with this connection
        let (last_connection, last_seen) = self.operate(|e| {
            if let Some((last_connection, connection_ts)) = e.last_connection() {
                if let Some(last_seen) = e.peer_stats().last_seen {
                    Some((last_connection, u64::max(last_seen, connection_ts)))
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
