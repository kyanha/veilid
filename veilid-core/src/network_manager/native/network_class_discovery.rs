use super::*;
use futures_util::stream::FuturesUnordered;
use futures_util::FutureExt;

struct DetectedPublicDialInfo {
    dial_info: DialInfo,
    class: DialInfoClass,
}
struct DiscoveryContextInner {
    // per-protocol
    intf_addrs: Option<Vec<SocketAddress>>,
    protocol_type: Option<ProtocolType>,
    address_type: Option<AddressType>,
    // first node contacted
    external_1_dial_info: Option<DialInfo>,
    external_1_address: Option<SocketAddress>,
    node_1: Option<NodeRef>,
    // detected public dialinfo
    detected_network_class: Option<NetworkClass>,
    detected_public_dial_info: Option<DetectedPublicDialInfo>,
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
                // per-protocol
                intf_addrs: None,
                protocol_type: None,
                address_type: None,
                external_1_dial_info: None,
                external_1_address: None,
                node_1: None,
                detected_network_class: None,
                detected_public_dial_info: None,
            })),
        }
    }

    ///////
    // Utilities

    // Pick the best network class we have seen so far
    pub fn set_detected_network_class(&self, network_class: NetworkClass) {
        let mut inner = self.inner.lock();
        log_net!( debug
            "=== set_detected_network_class {:?} {:?}: {:?} ===",
            inner.protocol_type,
            inner.address_type,
            network_class
        );

        inner.detected_network_class = Some(network_class);
    }

    pub fn set_detected_public_dial_info(&self, dial_info: DialInfo, class: DialInfoClass) {
        let mut inner = self.inner.lock();
        log_net!( debug
            "=== set_detected_public_dial_info {:?} {:?}: {} {:?} ===",
            inner.protocol_type,
            inner.address_type,
            dial_info,
            class
        );
        inner.detected_public_dial_info = Some(DetectedPublicDialInfo { dial_info, class });
    }

    // Ask for a public address check from a particular noderef
    // This is done over the normal port using RPC
    async fn request_public_address(&self, node_ref: NodeRef) -> Option<SocketAddress> {
        let rpc = self.routing_table.rpc_processor();
        rpc.rpc_call_status(node_ref.clone())
            .await
            .map_err(logthru_net!(
                "failed to get status answer from {:?}",
                node_ref
            ))
            .map(|sa| {
                let ret = sa.sender_info.socket_address;
                log_net!("request_public_address: {:?}", ret);
                ret
            })
            .unwrap_or(None)
    }

    // find fast peers with a particular address type, and ask them to tell us what our external address is
    // This is done over the normal port using RPC
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

    // This pulls the already-detected local interface dial info from the routing table
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
    ) -> bool {
        let rpc = self.routing_table.rpc_processor();
        rpc.rpc_call_validate_dial_info(node_ref.clone(), dial_info, redirect)
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
        inner.external_1_dial_info = None;
        inner.external_1_address = None;
        inner.node_1 = None;
    }

    // Get our first node's view of our external IP address via normal RPC
    pub async fn protocol_get_external_address_1(&self) -> bool {
        let (protocol_type, address_type) = {
            let inner = self.inner.lock();
            (inner.protocol_type.unwrap(), inner.address_type.unwrap())
        };

        // Get our external address from some fast node, call it node 1
        let (external_1, node_1) = match self
            .discover_external_address(protocol_type, address_type, None)
            .await
        {
            None => {
                // If we can't get an external address, exit but don't throw an error so we can try again later
                log_net!(debug "couldn't get external address 1 for {:?} {:?}", protocol_type, address_type);
                return false;
            }
            Some(v) => v,
        };
        let external_1_dial_info = self.make_dial_info(external_1, protocol_type);

        let mut inner = self.inner.lock();
        inner.external_1_dial_info = Some(external_1_dial_info);
        inner.external_1_address = Some(external_1);
        inner.node_1 = Some(node_1);

        log_net!(debug "external_1_dial_info: {:?}\nexternal_1_address: {:?}\nnode_1: {:?}", inner.external_1_dial_info, inner.external_1_address, inner.node_1);

        true
    }

    // If we know we are not behind NAT, check our firewall status
    pub async fn protocol_process_no_nat(&self) -> Result<(), String> {
        let (node_b, external_1_dial_info) = {
            let inner = self.inner.lock();
            (
                inner.node_1.as_ref().unwrap().clone(),
                inner.external_1_dial_info.as_ref().unwrap().clone(),
            )
        };

        // Do a validate_dial_info on the external address from a redirected node
        if self
            .validate_dial_info(node_b.clone(), external_1_dial_info.clone(), true)
            .await
        {
            // Add public dial info with Direct dialinfo class
            self.set_detected_public_dial_info(external_1_dial_info, DialInfoClass::Direct);
        }
        // Attempt a port mapping via all available and enabled mechanisms
        else if let Some(external_mapped_dial_info) = self.try_port_mapping().await {
            // Got a port mapping, let's use it
            self.set_detected_public_dial_info(external_mapped_dial_info, DialInfoClass::Mapped);
        } else {
            // Add public dial info with Blocked dialinfo class
            self.set_detected_public_dial_info(external_1_dial_info, DialInfoClass::Blocked);
        }
        self.set_detected_network_class(NetworkClass::InboundCapable);
        Ok(())
    }

    // If we know we are behind NAT check what kind
    pub async fn protocol_process_nat(&self) -> Result<bool, String> {
        let (node_1, external_1_dial_info, external_1_address, protocol_type, address_type) = {
            let inner = self.inner.lock();
            (
                inner.node_1.as_ref().unwrap().clone(),
                inner.external_1_dial_info.as_ref().unwrap().clone(),
                inner.external_1_address.unwrap(),
                inner.protocol_type.unwrap(),
                inner.address_type.unwrap(),
            )
        };

        // Attempt a UDP port mapping via all available and enabled mechanisms
        if let Some(external_mapped_dial_info) = self.try_port_mapping().await {
            // Got a port mapping, let's use it
            self.set_detected_public_dial_info(external_mapped_dial_info, DialInfoClass::Mapped);
            self.set_detected_network_class(NetworkClass::InboundCapable);

            // No more retries
            return Ok(true);
        }

        // Port mapping was not possible, let's see what kind of NAT we have

        // Does a redirected dial info validation from a different address and a random port find us?
        if self
            .validate_dial_info(node_1.clone(), external_1_dial_info.clone(), true)
            .await
        {
            // Yes, another machine can use the dial info directly, so Full Cone
            // Add public dial info with full cone NAT network class
            self.set_detected_public_dial_info(external_1_dial_info, DialInfoClass::FullConeNAT);
            self.set_detected_network_class(NetworkClass::InboundCapable);

            // No more retries
            return Ok(true);
        }

        // No, we are restricted, determine what kind of restriction

        // Get our external address from some fast node, that is not node 1, call it node 2
        let (external_2_address, node_2) = match self
            .discover_external_address(protocol_type, address_type, Some(node_1.node_id()))
            .await
        {
            None => {
                // If we can't get an external address, allow retry
                return Ok(false);
            }
            Some(v) => v,
        };

        // If we have two different external addresses, then this is a symmetric NAT
        if external_2_address != external_1_address {
            // Symmetric NAT is outbound only, no public dial info will work
            self.set_detected_network_class(NetworkClass::OutboundOnly);

            // No more retries
            return Ok(true);
        }

        // If we're going to end up as a restricted NAT of some sort
        // Address is the same, so it's address or port restricted

        // Do a validate_dial_info on the external address from a random port
        if self
            .validate_dial_info(node_2.clone(), external_1_dial_info.clone(), false)
            .await
        {
            // Got a reply from a non-default port, which means we're only address restricted
            self.set_detected_public_dial_info(
                external_1_dial_info,
                DialInfoClass::AddressRestrictedNAT,
            );
        } else {
            // Didn't get a reply from a non-default port, which means we are also port restricted
            self.set_detected_public_dial_info(
                external_1_dial_info,
                DialInfoClass::PortRestrictedNAT,
            );
        }
        self.set_detected_network_class(NetworkClass::InboundCapable);

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
            log_net!(debug
                "=== update_ipv4_protocol_dialinfo {:?} tries_left={} ===",
                protocol_type,
                retry_count
            );
            // Get our external address from some fast node, call it node 1
            if !context.protocol_get_external_address_1().await {
                // If we couldn't get an external address, then we should just try the whole network class detection again later
                return Ok(());
            }

            // If our local interface list contains external_1 then there is no NAT in place
            {
                let res = {
                    let inner = context.inner.lock();
                    inner
                        .intf_addrs
                        .as_ref()
                        .unwrap()
                        .contains(inner.external_1_address.as_ref().unwrap())
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

        log_net!(debug "=== update_ipv6_protocol_dialinfo {:?} ===", protocol_type);

        // Get our external address from some fast node, call it node 1
        if !context.protocol_get_external_address_1().await {
            // If we couldn't get an external address, then we should just try the whole network class detection again later
            return Ok(());
        }

        // If our local interface list doesn't contain external_1 then there is an Ipv6 NAT in place
        {
            let inner = context.inner.lock();
            if !inner
                .intf_addrs
                .as_ref()
                .unwrap()
                .contains(inner.external_1_address.as_ref().unwrap())
            {
                // IPv6 NAT is not supported today
                log_net!(warn
                    "IPv6 NAT is not supported for external address: {}",
                    inner.external_1_address.unwrap()
                );
                return Ok(());
            }
        }

        // No NAT
        context.protocol_process_no_nat().await?;

        Ok(())
    }

    pub async fn update_network_class_task_routine(self, _l: u64, _t: u64) -> Result<(), String> {
        log_net!("--- updating network class");

        // Ensure we aren't trying to update this without clearing it first
        let old_network_class = self.inner.lock().network_class;
        assert_eq!(old_network_class, None);

        let protocol_config = self.inner.lock().protocol_config.unwrap_or_default();
        let mut unord = FuturesUnordered::new();

        // Do UDPv4+v6 at the same time as everything else
        if protocol_config.inbound.contains(ProtocolType::UDP) {
            // UDPv4
            unord.push(
                async {
                    let udpv4_context = DiscoveryContext::new(self.routing_table(), self.clone());
                    if let Err(e) = self
                        .update_ipv4_protocol_dialinfo(&udpv4_context, ProtocolType::UDP)
                        .await
                    {
                        log_net!(debug "Failed UDPv4 dialinfo discovery: {}", e);
                        return None;
                    }
                    Some(vec![udpv4_context])
                }
                .boxed(),
            );

            // UDPv6
            unord.push(
                async {
                    let udpv6_context = DiscoveryContext::new(self.routing_table(), self.clone());
                    if let Err(e) = self
                        .update_ipv6_protocol_dialinfo(&udpv6_context, ProtocolType::UDP)
                        .await
                    {
                        log_net!(debug "Failed UDPv6 dialinfo discovery: {}", e);
                        return None;
                    }
                    Some(vec![udpv6_context])
                }
                .boxed(),
            );
        }

        // Do TCPv4 + WSv4 in series because they may use the same connection 5-tuple
        unord.push(
            async {
                // TCPv4
                let mut out = Vec::<DiscoveryContext>::new();
                if protocol_config.inbound.contains(ProtocolType::TCP) {
                    let tcpv4_context = DiscoveryContext::new(self.routing_table(), self.clone());
                    if let Err(e) = self
                        .update_ipv4_protocol_dialinfo(&tcpv4_context, ProtocolType::TCP)
                        .await
                    {
                        log_net!(debug "Failed TCPv4 dialinfo discovery: {}", e);
                        return None;
                    }
                    out.push(tcpv4_context);
                }

                // WSv4
                if protocol_config.inbound.contains(ProtocolType::WS) {
                    let wsv4_context = DiscoveryContext::new(self.routing_table(), self.clone());
                    if let Err(e) = self
                        .update_ipv4_protocol_dialinfo(&wsv4_context, ProtocolType::WS)
                        .await
                    {
                        log_net!(debug "Failed WSv4 dialinfo discovery: {}", e);
                        return None;
                    }
                    out.push(wsv4_context);
                }
                Some(out)
            }
            .boxed(),
        );

        // Do TCPv6 + WSv6 in series because they may use the same connection 5-tuple
        unord.push(
            async {
                // TCPv6
                let mut out = Vec::<DiscoveryContext>::new();
                if protocol_config.inbound.contains(ProtocolType::TCP) {
                    let tcpv6_context = DiscoveryContext::new(self.routing_table(), self.clone());
                    if let Err(e) = self
                        .update_ipv6_protocol_dialinfo(&tcpv6_context, ProtocolType::TCP)
                        .await
                    {
                        log_net!(debug "Failed TCPv6 dialinfo discovery: {}", e);
                        return None;
                    }
                    out.push(tcpv6_context);
                }

                // WSv6
                if protocol_config.inbound.contains(ProtocolType::WS) {
                    let wsv6_context = DiscoveryContext::new(self.routing_table(), self.clone());
                    if let Err(e) = self
                        .update_ipv6_protocol_dialinfo(&wsv6_context, ProtocolType::WS)
                        .await
                    {
                        log_net!(debug "Failed WSv6 dialinfo discovery: {}", e);
                        return None;
                    }
                    out.push(wsv6_context);
                }
                Some(out)
            }
            .boxed(),
        );

        // Wait for all discovery futures to complete and collect contexts
        let mut contexts = Vec::<DiscoveryContext>::new();
        let mut network_class = Option::<NetworkClass>::None;
        while let Some(ctxvec) = unord.next().await {
            if let Some(ctxvec) = ctxvec {
                for ctx in ctxvec {
                    if let Some(nc) = ctx.inner.lock().detected_network_class {
                        if let Some(last_nc) = network_class {
                            if nc < last_nc {
                                network_class = Some(nc);
                            }
                        } else {
                            network_class = Some(nc);
                        }
                    }

                    contexts.push(ctx);
                }
            }
        }

        // Get best network class
        if network_class.is_some() {
            // Update public dial info
            let routing_table = self.routing_table();
            for ctx in contexts {
                let inner = ctx.inner.lock();
                if let Some(pdi) = &inner.detected_public_dial_info {
                    if let Err(e) = routing_table.register_dial_info(
                        RoutingDomain::PublicInternet,
                        pdi.dial_info.clone(),
                        pdi.class,
                    ) {
                        log_net!(warn "Failed to register detected public dial info: {}", e);
                    }
                }
            }
            // Update network class
            self.inner.lock().network_class = network_class;
            log_net!(debug "network class changed to {:?}", network_class);

            // Send updates to everyone
            routing_table.send_node_info_updates();
        }

        Ok(())
    }
}
