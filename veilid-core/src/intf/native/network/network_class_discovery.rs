use super::*;

use crate::intf::*;
use crate::routing_table::*;
use crate::*;

struct DiscoveryContextInner {
    network_class: Option<NetworkClass>,
    // per-protocol
    intf_addrs: Option<Vec<SocketAddress>>,
    protocol_type: Option<ProtocolType>,
    address_type: Option<AddressType>,
    low_level_protocol_type: Option<ProtocolType>,
    external1_dial_info: Option<DialInfo>,
    external1: Option<SocketAddress>,
    node_b: Option<NodeRef>,
}

pub struct DiscoveryContext {
    routing_table: RoutingTable,
    net: Network,
    inner: Arc<Mutex<DiscoveryContextInner>>,
}

impl DiscoveryContext {
    pub fn new(routing_table: RoutingTable, net: Network) -> Self {
        Self {
            routing_table,
            net,
            inner: Arc::new(Mutex::new(DiscoveryContextInner {
                network_class: None,
                // per-protocol
                intf_addrs: None,
                protocol_type: None,
                address_type: None,
                low_level_protocol_type: None,
                external1_dial_info: None,
                external1: None,
                node_b: None,
            })),
        }
    }

    ///////
    // Utilities

    // Pick the best network class we have seen so far
    pub fn upgrade_network_class(&self, network_class: NetworkClass) {
        let mut inner = self.inner.lock();

        if let Some(old_nc) = inner.network_class {
            if network_class < old_nc {
                inner.network_class = Some(network_class);
            }
        } else {
            inner.network_class = Some(network_class);
        }
    }

    // Ask for a public address check from a particular noderef
    async fn request_public_address(&self, node_ref: NodeRef) -> Option<SocketAddress> {
        let rpc = self.routing_table.rpc_processor();
        rpc.rpc_call_status(node_ref.clone())
            .await
            .map_err(logthru_net!(
                "failed to get status answer from {:?}",
                node_ref
            ))
            .map(|sa| sa.sender_info.socket_address)
            .unwrap_or(None)
    }

    // find fast peers with a particular address type, and ask them to tell us what our external address is
    async fn discover_external_address(
        &self,
        protocol_type: ProtocolType,
        address_type: AddressType,
        ignore_node: Option<DHTKey>,
    ) -> Option<(SocketAddress, NodeRef)> {
        let filter = DialInfoFilter::global()
            .with_protocol_type(protocol_type)
            .with_address_type(address_type);
        let peers = self.routing_table.find_fast_public_nodes_filtered(&filter);
        if peers.is_empty() {
            log_net!("no peers of type '{:?}'", filter);
            return None;
        }
        for peer in peers {
            if let Some(ignore_node) = ignore_node {
                if peer.node_id() == ignore_node {
                    continue;
                }
            }
            if let Some(sa) = self.request_public_address(peer.clone()).await {
                return Some((sa, peer));
            }
        }
        log_net!("no peers responded with an external address");
        None
    }

    fn get_local_addresses(
        &self,
        protocol_type: ProtocolType,
        address_type: AddressType,
    ) -> Vec<SocketAddress> {
        let filter = DialInfoFilter::local()
            .with_protocol_type(protocol_type)
            .with_address_type(address_type);
        self.routing_table
            .dial_info_details(RoutingDomain::LocalNetwork)
            .iter()
            .filter_map(|did| {
                if did.dial_info.matches_filter(&filter) {
                    Some(did.dial_info.socket_address())
                } else {
                    None
                }
            })
            .collect()
    }

    async fn validate_dial_info(
        &self,
        node_ref: NodeRef,
        dial_info: DialInfo,
        redirect: bool,
        alternate_port: bool,
    ) -> bool {
        let rpc = self.routing_table.rpc_processor();
        rpc.rpc_call_validate_dial_info(node_ref.clone(), dial_info, redirect, alternate_port)
            .await
            .map_err(logthru_net!(
                "failed to send validate_dial_info to {:?}",
                node_ref
            ))
            .unwrap_or(false)
    }

    async fn try_port_mapping(&self) -> Option<DialInfo> {
        //xxx
        None
    }

    fn make_dial_info(&self, addr: SocketAddress, protocol_type: ProtocolType) -> DialInfo {
        match protocol_type {
            ProtocolType::UDP => DialInfo::udp(addr),
            ProtocolType::TCP => DialInfo::tcp(addr),
            ProtocolType::WS => {
                let c = self.net.config.get();
                DialInfo::try_ws(
                    addr,
                    format!("ws://{}/{}", addr, c.network.protocol.ws.path),
                )
                .unwrap()
            }
            ProtocolType::WSS => panic!("none of the discovery functions are used for wss"),
        }
    }

    ///////
    // Per-protocol discovery routines

    pub fn protocol_begin(&self, protocol_type: ProtocolType, address_type: AddressType) {
        // Get our interface addresses
        let intf_addrs = self.get_local_addresses(protocol_type, address_type);

        let mut inner = self.inner.lock();
        inner.intf_addrs = Some(intf_addrs);
        inner.protocol_type = Some(protocol_type);
        inner.address_type = Some(address_type);
        inner.low_level_protocol_type = Some(match protocol_type {
            ProtocolType::UDP => ProtocolType::UDP,
            ProtocolType::TCP => ProtocolType::TCP,
            ProtocolType::WS => ProtocolType::TCP,
            ProtocolType::WSS => ProtocolType::TCP,
        });
        inner.external1_dial_info = None;
        inner.external1 = None;
        inner.node_b = None;
    }

    pub async fn protocol_get_external_address_1(&self) -> bool {
        let (protocol_type, address_type) = {
            let inner = self.inner.lock();
            (inner.protocol_type.unwrap(), inner.address_type.unwrap())
        };

        // Get our external address from some fast node, call it node B
        let (external1, node_b) = match self
            .discover_external_address(protocol_type, address_type, None)
            .await
        {
            None => {
                // If we can't get an external address, exit but don't throw an error so we can try again later
                return false;
            }
            Some(v) => v,
        };
        let external1_dial_info = self.make_dial_info(external1, protocol_type);

        let mut inner = self.inner.lock();
        inner.external1_dial_info = Some(external1_dial_info);
        inner.external1 = Some(external1);
        inner.node_b = Some(node_b);

        true
    }

    pub async fn protocol_process_no_nat(&self) -> Result<(), String> {
        let (node_b, external1_dial_info) = {
            let inner = self.inner.lock();
            (
                inner.node_b.as_ref().unwrap().clone(),
                inner.external1_dial_info.as_ref().unwrap().clone(),
            )
        };

        // Do a validate_dial_info on the external address from a redirected node
        if self
            .validate_dial_info(node_b.clone(), external1_dial_info.clone(), true, false)
            .await
        {
            // Add public dial info with Direct dialinfo class
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external1_dial_info,
                DialInfoClass::Direct,
            )?;
        }
        // Attempt a UDP port mapping via all available and enabled mechanisms
        else if let Some(external_mapped_dial_info) = self.try_port_mapping().await {
            // Got a port mapping, let's use it
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external_mapped_dial_info,
                DialInfoClass::Mapped,
            )?;
        } else {
            // Add public dial info with Blocked dialinfo class
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external1_dial_info,
                DialInfoClass::Blocked,
            )?;
        }
        self.upgrade_network_class(NetworkClass::InboundCapable);
        Ok(())
    }

    pub async fn protocol_process_nat(&self) -> Result<bool, String> {
        let (node_b, external1_dial_info, external1, protocol_type, address_type) = {
            let inner = self.inner.lock();
            (
                inner.node_b.as_ref().unwrap().clone(),
                inner.external1_dial_info.as_ref().unwrap().clone(),
                inner.external1.unwrap(),
                inner.protocol_type.unwrap(),
                inner.address_type.unwrap(),
            )
        };

        // Attempt a UDP port mapping via all available and enabled mechanisms
        if let Some(external_mapped_dial_info) = self.try_port_mapping().await {
            // Got a port mapping, let's use it
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external_mapped_dial_info,
                DialInfoClass::Mapped,
            )?;
            self.upgrade_network_class(NetworkClass::InboundCapable);

            // No more retries
            return Ok(true);
        }

        // Port mapping was not possible, let's see what kind of NAT we have

        // Does a redirected dial info validation find us?
        if self
            .validate_dial_info(node_b.clone(), external1_dial_info.clone(), true, false)
            .await
        {
            // Yes, another machine can use the dial info directly, so Full Cone
            // Add public dial info with full cone NAT network class
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external1_dial_info,
                DialInfoClass::FullConeNAT,
            )?;
            self.upgrade_network_class(NetworkClass::InboundCapable);

            return Ok(true);
        }

        // No, we are restricted, determine what kind of restriction

        // Get our external address from some fast node, that is not node B, call it node D
        let (external2, node_d) = match self
            .discover_external_address(protocol_type, address_type, Some(node_b.node_id()))
            .await
        {
            None => {
                // If we can't get an external address, allow retry
                return Ok(false);
            }
            Some(v) => v,
        };

        // If we have two different external addresses, then this is a symmetric NAT
        if external2 != external1 {
            // Symmetric NAT is outbound only, no public dial info will work
            self.upgrade_network_class(NetworkClass::OutboundOnly);

            // No more retries
            return Ok(true);
        }

        // If we're going to end up as a restricted NAT of some sort

        // Address is the same, so it's address or port restricted
        let external2_dial_info = DialInfo::udp(external2);
        // Do a validate_dial_info on the external address from a routed node
        if self
            .validate_dial_info(node_d.clone(), external2_dial_info.clone(), false, true)
            .await
        {
            // Got a reply from a non-default port, which means we're only address restricted
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external1_dial_info,
                DialInfoClass::AddressRestrictedNAT,
            )?;
        } else {
            // Didn't get a reply from a non-default port, which means we are also port restricted
            self.routing_table.register_dial_info(
                RoutingDomain::PublicInternet,
                external1_dial_info,
                DialInfoClass::PortRestrictedNAT,
            )?;
        }
        self.upgrade_network_class(NetworkClass::InboundCapable);

        // Allow another retry because sometimes trying again will get us Full Cone NAT instead
        Ok(false)
    }
}

impl Network {
    pub async fn update_ipv4_protocol_dialinfo(
        &self,
        context: &DiscoveryContext,
        protocol_type: ProtocolType,
    ) -> Result<(), String> {
        let mut retry_count = {
            let c = self.config.get();
            c.network.restricted_nat_retries
        };

        // Start doing ipv4 protocol
        context.protocol_begin(protocol_type, AddressType::IPV4);

        // Loop for restricted NAT retries
        loop {
            // Get our external address from some fast node, call it node B
            if !context.protocol_get_external_address_1().await {
                // If we couldn't get an external address, then we should just try the whole network class detection again later
                return Ok(());
            }

            // If our local interface list contains external1 then there is no NAT in place
            {
                let res = {
                    let inner = context.inner.lock();
                    inner
                        .intf_addrs
                        .as_ref()
                        .unwrap()
                        .contains(inner.external1.as_ref().unwrap())
                };
                if res {
                    // No NAT
                    context.protocol_process_no_nat().await?;

                    // No more retries
                    break;
                }
            }

            // There is -some NAT-
            if context.protocol_process_nat().await? {
                // We either got dial info or a network class without one
                break;
            }

            // If we tried everything, break anyway after N attempts
            if retry_count == 0 {
                break;
            }
            retry_count -= 1;
        }

        Ok(())
    }

    pub async fn update_ipv6_protocol_dialinfo(
        &self,
        context: &DiscoveryContext,
        protocol_type: ProtocolType,
    ) -> Result<(), String> {
        // Start doing ipv6 protocol
        context.protocol_begin(protocol_type, AddressType::IPV6);

        // Get our external address from some fast node, call it node B
        if !context.protocol_get_external_address_1().await {
            // If we couldn't get an external address, then we should just try the whole network class detection again later
            return Ok(());
        }

        // If our local interface list doesn't contain external1 then there is an Ipv6 NAT in place
        {
            let inner = context.inner.lock();
            if !inner
                .intf_addrs
                .as_ref()
                .unwrap()
                .contains(inner.external1.as_ref().unwrap())
            {
                // IPv6 NAT is not supported today
                log_net!(warn
                    "IPv6 NAT is not supported for external address: {}",
                    inner.external1.unwrap()
                );
                return Ok(());
            }
        }

        // No NAT
        context.protocol_process_no_nat().await?;

        Ok(())
    }

    pub async fn update_network_class_task_routine(self, _l: u64, _t: u64) -> Result<(), String> {
        log_net!("updating network class");

        let protocol_config = self.inner.lock().protocol_config.unwrap_or_default();

        let context = DiscoveryContext::new(self.routing_table(), self.clone());

        if protocol_config.inbound.contains(ProtocolType::UDP) {
            self.update_ipv4_protocol_dialinfo(&context, ProtocolType::UDP)
                .await?;
            self.update_ipv6_protocol_dialinfo(&context, ProtocolType::UDP)
                .await?;
        }

        if protocol_config.inbound.contains(ProtocolType::TCP) {
            self.update_ipv4_protocol_dialinfo(&context, ProtocolType::TCP)
                .await?;
            self.update_ipv6_protocol_dialinfo(&context, ProtocolType::TCP)
                .await?;
        }

        if protocol_config.inbound.contains(ProtocolType::WS) {
            self.update_ipv4_protocol_dialinfo(&context, ProtocolType::WS)
                .await?;
            self.update_ipv6_protocol_dialinfo(&context, ProtocolType::WS)
                .await?;
        }

        let network_class = context.inner.lock().network_class;
        self.inner.lock().network_class = network_class;

        log_net!(debug "network class set to {:?}", network_class);

        Ok(())
    }
}
