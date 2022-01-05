use crate::network_connection::*;
use crate::xx::*;
use crate::*;
use alloc::collections::btree_map::Entry;

#[derive(Debug)]
pub struct ConnectionTable {
    conn_by_descriptor: BTreeMap<ConnectionDescriptor, NetworkConnection>,
    conns_by_remote: BTreeMap<PeerAddress, Vec<NetworkConnection>>,
}

impl ConnectionTable {
    pub fn new() -> Self {
        Self {
            conn_by_descriptor: BTreeMap::new(),
            conns_by_remote: BTreeMap::new(),
        }
    }

    pub fn add_connection(&mut self, conn: NetworkConnection) -> Result<(), String> {
        let descriptor = conn.connection_descriptor();
        assert_ne!(
            descriptor.protocol_type(),
            ProtocolType::UDP,
            "Only connection oriented protocols go in the table!"
        );
        if self.conn_by_descriptor.contains_key(&descriptor) {
            return Err(format!(
                "Connection already added to table: {:?}",
                descriptor
            ));
        }
        let res = self.conn_by_descriptor.insert(descriptor, conn.clone());
        assert!(res.is_none());

        let conns = self.conns_by_remote.entry(descriptor.remote).or_default();
        //warn!("add_connection: {:?}", conn);
        conns.push(conn);

        Ok(())
    }

    pub fn get_connection(&self, descriptor: ConnectionDescriptor) -> Option<NetworkConnection> {
        let out = self.conn_by_descriptor.get(&descriptor).cloned();
        //warn!("get_connection: {:?} -> {:?}", descriptor, out);
        out
    }
    pub fn get_last_connection_by_remote(&self, remote: PeerAddress) -> Option<NetworkConnection> {
        let out = self
            .conns_by_remote
            .get(&remote)
            .map(|v| v[(v.len() - 1)].clone());
        //warn!("get_last_connection_by_remote: {:?} -> {:?}", remote, out);
        out
    }

    pub fn connection_count(&self) -> usize {
        self.conn_by_descriptor.len()
    }

    pub fn remove_connection(
        &mut self,
        descriptor: ConnectionDescriptor,
    ) -> Result<NetworkConnection, String> {
        //warn!("remove_connection: {:?}", descriptor);
        let out = self
            .conn_by_descriptor
            .remove(&descriptor)
            .ok_or_else(|| format!("Connection not in table: {:?}", descriptor))?;

        match self.conns_by_remote.entry(descriptor.remote) {
            Entry::Vacant(_) => {
                panic!("inconsistency in connection table")
            }
            Entry::Occupied(mut o) => {
                let v = o.get_mut();

                // Remove one matching connection from the list
                for (n, elem) in v.iter().enumerate() {
                    if elem.connection_descriptor() == descriptor {
                        v.remove(n);
                        break;
                    }
                }
                // No connections left for this remote, remove the entry from conns_by_remote
                if v.is_empty() {
                    o.remove_entry();
                }
            }
        }

        Ok(out)
    }
}
