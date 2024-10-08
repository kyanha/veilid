use super::*;

pub type Capability = FourCC;
pub const CAP_ROUTE: Capability = FourCC(*b"ROUT");
#[cfg(feature = "unstable-tunnels")]
pub const CAP_TUNNEL: Capability = FourCC(*b"TUNL");
pub const CAP_SIGNAL: Capability = FourCC(*b"SGNL");
pub const CAP_RELAY: Capability = FourCC(*b"RLAY");
pub const CAP_VALIDATE_DIAL_INFO: Capability = FourCC(*b"DIAL");
pub const CAP_DHT: Capability = FourCC(*b"DHTV");
pub const CAP_DHT_WATCH: Capability = FourCC(*b"DHTW");
pub const CAP_APPMESSAGE: Capability = FourCC(*b"APPM");
#[cfg(feature = "unstable-blockstore")]
pub const CAP_BLOCKSTORE: Capability = FourCC(*b"BLOC");

pub const DISTANCE_METRIC_CAPABILITIES: &[Capability] = &[CAP_DHT, CAP_DHT_WATCH];

#[derive(Clone, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    network_class: NetworkClass,
    outbound_protocols: ProtocolTypeSet,
    address_types: AddressTypeSet,
    envelope_support: Vec<u8>,
    crypto_support: Vec<CryptoKind>,
    capabilities: Vec<Capability>,
    dial_info_detail_list: Vec<DialInfoDetail>,
}

impl NodeInfo {
    pub fn new(
        network_class: NetworkClass,
        outbound_protocols: ProtocolTypeSet,
        address_types: AddressTypeSet,
        envelope_support: Vec<u8>,
        crypto_support: Vec<CryptoKind>,
        capabilities: Vec<Capability>,
        dial_info_detail_list: Vec<DialInfoDetail>,
    ) -> Self {
        Self {
            network_class,
            outbound_protocols,
            address_types,
            envelope_support,
            crypto_support,
            capabilities,
            dial_info_detail_list,
        }
    }

    pub fn network_class(&self) -> NetworkClass {
        self.network_class
    }
    pub fn outbound_protocols(&self) -> ProtocolTypeSet {
        self.outbound_protocols
    }
    pub fn address_types(&self) -> AddressTypeSet {
        self.address_types
    }
    pub fn envelope_support(&self) -> &[u8] {
        &self.envelope_support
    }
    pub fn crypto_support(&self) -> &[CryptoKind] {
        &self.crypto_support
    }
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }
    pub fn dial_info_detail_list(&self) -> &[DialInfoDetail] {
        &self.dial_info_detail_list
    }

    pub fn first_filtered_dial_info_detail<S, F>(
        &self,
        sort: Option<S>,
        filter: F,
    ) -> Option<DialInfoDetail>
    where
        S: Fn(&DialInfoDetail, &DialInfoDetail) -> std::cmp::Ordering,
        F: Fn(&DialInfoDetail) -> bool,
    {
        if let Some(sort) = sort {
            let mut dids = self.dial_info_detail_list.clone();
            dids.sort_by(sort);
            for did in dids {
                if filter(&did) {
                    return Some(did);
                }
            }
        } else {
            for did in &self.dial_info_detail_list {
                if filter(did) {
                    return Some(did.clone());
                }
            }
        };
        None
    }

    pub fn filtered_dial_info_details<S, F>(
        &self,
        sort: Option<S>,
        filter: F,
    ) -> Vec<DialInfoDetail>
    where
        S: Fn(&DialInfoDetail, &DialInfoDetail) -> std::cmp::Ordering,
        F: Fn(&DialInfoDetail) -> bool,
    {
        let mut dial_info_detail_list = Vec::new();

        if let Some(sort) = sort {
            let mut dids = self.dial_info_detail_list.clone();
            dids.sort_by(sort);
            for did in dids {
                if filter(&did) {
                    dial_info_detail_list.push(did);
                }
            }
        } else {
            for did in &self.dial_info_detail_list {
                if filter(did) {
                    dial_info_detail_list.push(did.clone());
                }
            }
        };
        dial_info_detail_list
    }

    /// Does this node has some dial info
    pub fn has_dial_info(&self) -> bool {
        !self.dial_info_detail_list.is_empty()
    }

    /// Is some relay required either for signal or inbound relay or outbound relay?
    pub fn requires_relay(&self) -> Option<RelayKind> {
        match self.network_class {
            NetworkClass::InboundCapable => {
                for did in &self.dial_info_detail_list {
                    if did.class.requires_relay() {
                        return Some(RelayKind::Inbound);
                    }
                }
            }
            NetworkClass::OutboundOnly => {
                return Some(RelayKind::Inbound);
            }
            NetworkClass::WebApp => {
                return Some(RelayKind::Outbound);
            }
            NetworkClass::Invalid => {}
        }
        None
    }

    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
    pub fn has_all_capabilities(&self, capabilities: &[Capability]) -> bool {
        for cap in capabilities {
            if !self.has_capability(*cap) {
                return false;
            }
        }
        true
    }
    pub fn has_any_capabilities(&self, capabilities: &[Capability]) -> bool {
        if capabilities.is_empty() {
            return true;
        }
        for cap in capabilities {
            if self.has_capability(*cap) {
                return true;
            }
        }
        false
    }

    /// Can direct connections be made
    pub fn is_fully_direct_inbound(&self) -> bool {
        // Must be inbound capable
        if !matches!(self.network_class, NetworkClass::InboundCapable) {
            return false;
        }
        // Do any of our dial info require signalling? if so, we can't offer signalling
        for did in &self.dial_info_detail_list {
            if did.class.requires_signal() {
                return false;
            }
        }
        true
    }

    /// Does this appear on the same network within the routing domain
    pub fn node_is_on_same_ipblock(&self, node_b: &NodeInfo, ip6_prefix_size: usize) -> bool {
        let our_ip_blocks = self
            .dial_info_detail_list()
            .iter()
            .map(|did| ip_to_ipblock(ip6_prefix_size, did.dial_info.to_socket_addr().ip()))
            .collect::<HashSet<_>>();

        for did in node_b.dial_info_detail_list() {
            let ipblock = ip_to_ipblock(ip6_prefix_size, did.dial_info.to_socket_addr().ip());
            if our_ip_blocks.contains(&ipblock) {
                return true;
            }
        }
        false
    }
}
