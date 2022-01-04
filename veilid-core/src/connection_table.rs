use crate::network_connection::*;
use crate::xx::*;
use crate::*;

#[derive(Debug)]
pub struct ConnectionTable {
    conn_by_addr: BTreeMap<ConnectionDescriptor, NetworkConnection>,
}

impl ConnectionTable {
    pub fn new() -> Self {
        Self {
            conn_by_addr: BTreeMap::new(),
        }
    }

    pub fn add_connection(&mut self, conn: NetworkConnection) -> Result<(), String> {
        let descriptor = conn.connection_descriptor();
        assert_ne!(
            descriptor.protocol_type(),
            ProtocolType::UDP,
            "Only connection oriented protocols go in the table!"
        );
        if self.conn_by_addr.contains_key(&descriptor) {
            return Err(format!(
                "Connection already added to table: {:?}",
                descriptor
            ));
        }
        let res = self.conn_by_addr.insert(descriptor, conn);
        assert!(res.is_none());
        Ok(())
    }

    pub fn get_connection(&self, descriptor: ConnectionDescriptor) -> Option<NetworkConnection> {
        self.conn_by_addr.get(&descriptor).cloned()
    }

    pub fn connection_count(&self) -> usize {
        self.conn_by_addr.len()
    }

    pub fn remove_connection(
        &mut self,
        descriptor: ConnectionDescriptor,
    ) -> Result<NetworkConnection, String> {
        self.conn_by_addr
            .remove(&descriptor)
            .ok_or_else(|| format!("Connection not in table: {:?}", descriptor))
    }
}
