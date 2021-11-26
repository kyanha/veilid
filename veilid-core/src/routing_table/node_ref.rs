use super::*;
use crate::dht::*;
use alloc::fmt;

pub struct NodeRef {
    routing_table: RoutingTable,
    node_id: DHTKey,
    protocol_address_type: Option<ProtocolAddressType>,
}

impl NodeRef {
    pub fn new(routing_table: RoutingTable, key: DHTKey, entry: &mut BucketEntry) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
            protocol_address_type: None,
        }
    }
    pub fn new_filtered(
        routing_table: RoutingTable,
        key: DHTKey,
        entry: &mut BucketEntry,
        protocol_address_type: ProtocolAddressType,
    ) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
            protocol_address_type: Some(protocol_address_type),
        }
    }

    pub fn node_id(&self) -> DHTKey {
        self.node_id
    }

    pub fn protocol_address_type(&self) -> Option<ProtocolAddressType> {
        self.protocol_address_type
    }

    pub fn set_protocol_address_type(
        &mut self,
        protocol_address_type: Option<ProtocolAddressType>,
    ) {
        self.protocol_address_type = protocol_address_type;
    }

    pub fn operate<T, F>(&self, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        self.routing_table.operate_on_bucket_entry(self.node_id, f)
    }

    pub fn dial_info(&self) -> Option<DialInfo> {
        match self.protocol_address_type {
            None => self.operate(|e| e.best_dial_info()),
            Some(pat) => self.operate(|e| {
                e.filtered_dial_info(|die| die.dial_info().protocol_address_type() == pat)
            }),
        }
    }
    pub fn last_connection(&self) -> Option<ConnectionDescriptor> {
        match self.operate(|e| e.last_connection()) {
            None => None,
            Some(c) => {
                if let Some(protocol_address_type) = self.protocol_address_type {
                    if c.remote.protocol_address_type() == protocol_address_type {
                        Some(c)
                    } else {
                        None
                    }
                } else {
                    Some(c)
                }
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
            protocol_address_type: self.protocol_address_type,
        }
    }
}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.protocol_address_type {
            None => write!(f, "{}", self.node_id.encode()),
            Some(pat) => write!(f, "{}#{:?}", self.node_id.encode(), pat),
        }
    }
}

impl Drop for NodeRef {
    fn drop(&mut self) {
        self.routing_table.drop_node_ref(self.node_id);
    }
}
