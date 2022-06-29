use super::*;
use alloc::collections::btree_map::Entry;
use futures_util::StreamExt;
use hashlink::LruCache;

#[derive(Debug)]
pub struct ConnectionTable {
    max_connections: Vec<usize>,
    conn_by_descriptor: Vec<LruCache<ConnectionDescriptor, NetworkConnection>>,
    descriptors_by_remote: BTreeMap<PeerAddress, Vec<ConnectionDescriptor>>,
    address_filter: ConnectionLimits,
}

fn protocol_to_index(protocol: ProtocolType) -> usize {
    match protocol {
        ProtocolType::TCP => 0,
        ProtocolType::WS => 1,
        ProtocolType::WSS => 2,
        ProtocolType::UDP => panic!("not a connection-oriented protocol"),
    }
}

impl ConnectionTable {
    pub fn new(config: VeilidConfig) -> Self {
        let max_connections = {
            let c = config.get();
            vec![
                c.network.protocol.tcp.max_connections as usize,
                c.network.protocol.ws.max_connections as usize,
                c.network.protocol.wss.max_connections as usize,
            ]
        };
        Self {
            max_connections,
            conn_by_descriptor: vec![
                LruCache::new_unbounded(),
                LruCache::new_unbounded(),
                LruCache::new_unbounded(),
            ],
            descriptors_by_remote: BTreeMap::new(),
            address_filter: ConnectionLimits::new(config),
        }
    }

    pub async fn join(&mut self) {
        let mut unord = FuturesUnordered::new();
        for table in &mut self.conn_by_descriptor {
            for (_, v) in table.drain() {
                trace!("connection table join: {:?}", v);
                unord.push(v);
            }
        }
        while unord.next().await.is_some() {}
    }

    pub fn add_connection(&mut self, conn: NetworkConnection) -> Result<(), String> {
        let descriptor = conn.connection_descriptor();
        let ip_addr = descriptor.remote_address().to_ip_addr();

        let index = protocol_to_index(descriptor.protocol_type());
        if self.conn_by_descriptor[index].contains_key(&descriptor) {
            return Err(format!(
                "Connection already added to table: {:?}",
                descriptor
            ));
        }

        // Filter by ip for connection limits
        self.address_filter.add(ip_addr).map_err(map_to_string)?;

        // Add the connection to the table
        let res = self.conn_by_descriptor[index].insert(descriptor.clone(), conn);
        assert!(res.is_none());

        // if we have reached the maximum number of connections per protocol type
        // then drop the least recently used connection
        if self.conn_by_descriptor[index].len() > self.max_connections[index] {
            if let Some((lruk, _)) = self.conn_by_descriptor[index].remove_lru() {
                debug!("connection lru out: {:?}", lruk);
                self.remove_connection_records(lruk);
            }
        }

        // add connection records
        let descriptors = self
            .descriptors_by_remote
            .entry(descriptor.remote())
            .or_default();

        descriptors.push(descriptor);

        Ok(())
    }

    pub fn get_connection(&mut self, descriptor: ConnectionDescriptor) -> Option<ConnectionHandle> {
        let index = protocol_to_index(descriptor.protocol_type());
        let out = self.conn_by_descriptor[index].get(&descriptor);
        out.map(|c| c.get_handle())
    }

    pub fn get_last_connection_by_remote(
        &mut self,
        remote: PeerAddress,
    ) -> Option<ConnectionHandle> {
        let descriptor = self
            .descriptors_by_remote
            .get(&remote)
            .map(|v| v[(v.len() - 1)].clone());
        if let Some(descriptor) = descriptor {
            // lru bump
            let index = protocol_to_index(descriptor.protocol_type());
            let handle = self.conn_by_descriptor[index]
                .get(&descriptor)
                .map(|c| c.get_handle());
            handle
        } else {
            None
        }
    }

    pub fn get_connection_descriptors_by_remote(
        &mut self,
        remote: PeerAddress,
    ) -> Vec<ConnectionDescriptor> {
        self.descriptors_by_remote
            .get(&remote)
            .cloned()
            .unwrap_or_default()
    }

    pub fn connection_count(&self) -> usize {
        self.conn_by_descriptor.iter().fold(0, |b, c| b + c.len())
    }

    fn remove_connection_records(&mut self, descriptor: ConnectionDescriptor) {
        let ip_addr = descriptor.remote_address().to_ip_addr();

        // conns_by_remote
        match self.descriptors_by_remote.entry(descriptor.remote()) {
            Entry::Vacant(_) => {
                panic!("inconsistency in connection table")
            }
            Entry::Occupied(mut o) => {
                let v = o.get_mut();

                // Remove one matching connection from the list
                for (n, elem) in v.iter().enumerate() {
                    if *elem == descriptor {
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
        self.address_filter
            .remove(ip_addr)
            .expect("Inconsistency in connection table");
    }

    pub fn remove_connection(
        &mut self,
        descriptor: ConnectionDescriptor,
    ) -> Result<NetworkConnection, String> {
        let index = protocol_to_index(descriptor.protocol_type());
        let conn = self.conn_by_descriptor[index]
            .remove(&descriptor)
            .ok_or_else(|| format!("Connection not in table: {:?}", descriptor))?;

        self.remove_connection_records(descriptor);
        Ok(conn)
    }
}
