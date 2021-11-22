use crate::intf::*;
use crate::xx::*;
use crate::*;

#[derive(Clone, Debug)]
pub struct ConnectionTableEntry {
    pub conn: NetworkConnection,
    pub established_time: u64,
    pub last_message_sent_time: Option<u64>,
    pub last_message_recv_time: Option<u64>,
    pub stopper: Eventual,
}

impl PartialEq for ConnectionTableEntry {
    fn eq(&self, other: &ConnectionTableEntry) -> bool {
        if self.conn != other.conn {
            return false;
        }
        if self.established_time != other.established_time {
            return false;
        }
        if self.last_message_sent_time != other.last_message_sent_time {
            return false;
        }
        if self.last_message_recv_time != other.last_message_recv_time {
            return false;
        }
        return true;
    }
}

#[derive(Debug)]
pub struct ConnectionTableInner {
    conn_by_addr: BTreeMap<ConnectionDescriptor, ConnectionTableEntry>,
}

#[derive(Clone)]
pub struct ConnectionTable {
    inner: Arc<Mutex<ConnectionTableInner>>,
}
impl core::fmt::Debug for ConnectionTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectionTable")
            .field("inner", &*self.inner.lock())
            .finish()
    }
}

impl ConnectionTable {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConnectionTableInner {
                conn_by_addr: BTreeMap::new(),
            })),
        }
    }

    pub fn add_connection(
        &self,
        descriptor: ConnectionDescriptor,
        conn: NetworkConnection,
    ) -> Result<ConnectionTableEntry, ()> {
        assert_ne!(
            descriptor.protocol_type(),
            ProtocolType::UDP,
            "Only connection oriented protocols go in the table!"
        );

        let mut inner = self.inner.lock();
        if inner.conn_by_addr.contains_key(&descriptor) {
            return Err(());
        }

        let timestamp = get_timestamp();

        let entry = ConnectionTableEntry {
            conn: conn,
            established_time: timestamp,
            last_message_sent_time: None,
            last_message_recv_time: None,
            stopper: Eventual::new(),
        };
        let res = inner.conn_by_addr.insert(descriptor, entry.clone());
        assert!(res.is_none());
        Ok(entry)
    }

    pub fn get_connection(
        &self,
        descriptor: &ConnectionDescriptor,
    ) -> Option<ConnectionTableEntry> {
        let inner = self.inner.lock();
        match inner.conn_by_addr.get(&descriptor) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub fn connection_count(&self) -> usize {
        let inner = self.inner.lock();
        inner.conn_by_addr.len()
    }

    pub fn remove_connection(
        &self,
        descriptor: &ConnectionDescriptor,
    ) -> Result<ConnectionTableEntry, ()> {
        let mut inner = self.inner.lock();

        let res = inner.conn_by_addr.remove(&descriptor);
        match res {
            Some(v) => Ok(v.clone()),
            None => Err(()),
        }
    }
}
