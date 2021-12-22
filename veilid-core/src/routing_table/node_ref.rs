use super::*;
use crate::dht::*;
use alloc::fmt;

pub struct NodeRef {
    routing_table: RoutingTable,
    node_id: DHTKey,
    // Filters
    protocol_type: Option<ProtocolType>,
    address_type: Option<AddressType>,
}

impl NodeRef {
    pub fn new(routing_table: RoutingTable, key: DHTKey, entry: &mut BucketEntry) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
            protocol_type: None,
            address_type: None,
        }
    }
    pub fn new_filtered(
        routing_table: RoutingTable,
        key: DHTKey,
        entry: &mut BucketEntry,
        protocol_type: Option<ProtocolType>,
        address_type: Option<AddressType>,
    ) -> Self {
        entry.ref_count += 1;
        Self {
            routing_table,
            node_id: key,
            protocol_type,
            address_type,
        }
    }

    pub fn node_id(&self) -> DHTKey {
        self.node_id
    }

    pub fn protocol_type(&self) -> Option<ProtocolType> {
        self.protocol_type
    }

    pub fn set_protocol_type(&mut self, protocol_type: Option<ProtocolType>) {
        self.protocol_type = protocol_type;
    }

    pub fn address_type(&self) -> Option<AddressType> {
        self.address_type
    }

    pub fn set_address_type(&mut self, address_type: Option<AddressType>) {
        self.address_type = address_type;
    }

    pub fn operate<T, F>(&self, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        self.routing_table.operate_on_bucket_entry(self.node_id, f)
    }

    xxx fix the notion of 'best dial info' to sort by capability and udp/tcp/ws/wss preference order
    pub fn dial_info(&self) -> Option<DialInfo> {
        if self.protocol_type || self. {
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
