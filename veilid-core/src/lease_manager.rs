use crate::*;
use network_manager::*;
use xx::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LeaseKind {
    Signal,
    Relay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelayMode {
    Disabled,
    Inbound,
    Full,
}

pub struct LeaseDetails {}

pub struct LeaseManagerInner {
    network_manager: NetworkManager,
    max_server_signal_leases: usize,
    max_server_relay_leases: usize,
    max_client_signal_leases: usize,
    max_client_relay_leases: usize,
    // server_signal_leases: BTreeMap< //xxx :how will these be accounted for?
    client_relay_mode: RelayMode,
}

#[derive(Clone)]
pub struct LeaseManager {
    inner: Arc<Mutex<LeaseManagerInner>>,
}

impl LeaseManager {
    fn new_inner(network_manager: NetworkManager) -> LeaseManagerInner {
        LeaseManagerInner {
            network_manager,
            max_server_signal_leases: 1,
            max_server_relay_leases: 1,
            max_client_signal_leases: 1,
            max_client_relay_leases: 1,
            client_relay_mode: RelayMode::Disabled,
        }
    }

    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
        }
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }
    pub async fn startup(&self) -> Result<(), String> {
        // Retrieve config
        {
            let mut inner = self.inner.lock();
            let config = inner.network_manager.config();
            let c = config.get();
            inner.max_server_signal_leases = c.network.leases.max_server_signal_leases as usize;
            inner.max_server_relay_leases = c.network.leases.max_server_relay_leases as usize;
            inner.max_client_signal_leases = c.network.leases.max_client_signal_leases as usize;
            inner.max_client_relay_leases = c.network.leases.max_client_relay_leases as usize;
        }

        Ok(())
    }
    pub async fn tick(&self) -> Result<(), String> {
        //
        Ok(())
    }
    pub async fn shutdown(&self) {
        let network_manager = self.network_manager();
        *self.inner.lock() = Self::new_inner(network_manager);
    }

    ////////////////////////////////
    // Client-side

    // xxx: this should automatically get set when a lease is obtained and reset when it is released or lost or expires
    // pub fn client_set_relay_mode(&self, relay_mode: RelayMode) {
    //     self.inner.lock().client_relay_mode = relay_mode;
    // }

    pub fn client_get_relay_mode(&self) -> RelayMode {
        self.inner.lock().client_relay_mode
    }

    pub fn client_is_relay_peer_addr(&self, peer_addr: PeerAddress) -> bool {
        error!("unimplemented");
        false
    }

    pub async fn client_request_lease(&self) -> Result<(), String> {
        Ok(())
    }

    ////////////////////////////////
    // Server-side

    // Signal leases
    pub fn server_has_valid_signal_lease(&self, recipient_id: &DHTKey) -> bool {
        error!("unimplemented");
        false
    }
    pub fn server_can_provide_signal_lease(&self) -> bool {
        let inner = self.inner.lock();
        if inner.max_server_signal_leases == 0 {
            return false;
        }
        let network_class = inner.network_manager.get_network_class();
        match network_class {
            NetworkClass::Server => true,
            NetworkClass::Mapped => true,
            NetworkClass::FullNAT => true,
            NetworkClass::AddressRestrictedNAT => false,
            NetworkClass::PortRestrictedNAT => false,
            NetworkClass::OutboundOnly => false,
            NetworkClass::WebApp => false,
            NetworkClass::TorWebApp => false,
            NetworkClass::Invalid => false,
        }
    }
    pub fn server_will_provide_signal_lease(&self) -> bool {
        if !self.server_can_provide_signal_lease() {
            return false;
        }
        let inner = self.inner.lock();
        if inner.max_server_signal_leases == 0 {
            return false;
        }
        // xxx: check total number of signal leases active...
        // xxx: depends on who is asking?
        // signaling requires inbound ability, so check to see if we have public dial info
        let routing_table = inner.network_manager.routing_table();
        if !routing_table.has_public_dial_info() {
            return false;
        }

        true
    }

    // Relay leases
    pub fn server_has_valid_relay_lease(&self, recipient_id: &DHTKey) -> bool {
        error!("unimplemented");
        false
    }
    pub fn server_can_provide_relay_lease(&self) -> bool {
        let inner = self.inner.lock();
        if inner.max_server_signal_leases == 0 {
            return false;
        }
        let network_class = inner.network_manager.get_network_class();
        match network_class {
            NetworkClass::Server => true,
            NetworkClass::Mapped => true,
            NetworkClass::FullNAT => true,
            NetworkClass::AddressRestrictedNAT => false,
            NetworkClass::PortRestrictedNAT => false,
            NetworkClass::OutboundOnly => false,
            NetworkClass::WebApp => false,
            NetworkClass::TorWebApp => false,
            NetworkClass::Invalid => false,
        }
        // xxx: also depends on network strength / bandwidth availability?
    }
    pub fn server_will_provide_relay_lease(&self) -> bool {
        if !self.server_can_provide_relay_lease() {
            return false;
        }
        let inner = self.inner.lock();
        if inner.max_server_relay_leases == 0 {
            return false;
        }
        // xxx: check total number of signal leases active...
        // xxx: depends on who is asking?
        // relaying requires inbound ability, so check to see if we have public dial info
        let routing_table = inner.network_manager.routing_table();
        if !routing_table.has_public_dial_info() {
            return false;
        }
        true
    }
}
