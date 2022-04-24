use super::*;

use crate::intf::*;
use crate::routing_table::*;
use crate::*;

#[derive(Debug)]
struct DiscoveryContext {
    routing_table: RoutingTable,
    external_ipv4: Option<Ipv4Addr>,
    external_ipv6: Option<Ipv6Addr>,
    network_class: Option<NetworkClass>,
}

impl DiscoveryContext {
    pub fn new(routing_table: RoutingTable) -> Self {
        Self {
            routing_table,
            external_ipv4: None,
            external_ipv6: None,
            network_class: None,
        }
    }
    pub fn upgrade_network_class(&mut self, network_class: NetworkClass) {
        if let Some(old_nc) = self.network_class {
            if network_class < old_nc {
                self.network_class = Some(network_class);
            }
        } else {
            self.network_class = Some(network_class);
        }
    }
}

impl Network {
    // Ask for a public address check from a particular noderef
    async fn request_public_address(&self, node_ref: NodeRef) -> Option<SocketAddress> {
        let routing_table = self.routing_table();
        let rpc = routing_table.rpc_processor();
        rpc.rpc_call_info(node_ref.clone())
            .await
            .map_err(logthru_net!(
                "failed to get info answer from {:?}",
                node_ref
            ))
            .map(|info_answer| info_answer.sender_info.socket_address)
            .unwrap_or(None)
    }

    // find fast peers with a particular address type, and ask them to tell us what our external address is
    async fn discover_external_address(
        &self,
        protocol_type: ProtocolType,
        address_type: AddressType,
        ignore_node: Option<DHTKey>,
    ) -> Option<(SocketAddress, NodeRef)> {
        let routing_table = self.routing_table();
        let filter = DialInfoFilter::global()
            .with_protocol_type(protocol_type)
            .with_address_type(address_type);
        let peers = routing_table.find_fast_public_nodes_filtered(&filter);
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
        let routing_table = self.routing_table();

        let filter = DialInfoFilter::local()
            .with_protocol_type(protocol_type)
            .with_address_type(address_type);
        routing_table
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
        let routing_table = self.routing_table();
        let rpc = routing_table.rpc_processor();
        rpc.rpc_call_validate_dial_info(node_ref.clone(), dial_info, redirect, alternate_port)
            .await
            .map_err(logthru_net!(
                "failed to send validate_dial_info to {:?}",
                node_ref
            ))
            .unwrap_or(false)
    }

    async fn try_port_mapping<I: AsRef<[SocketAddress]>>(
        &self,
        _intf_addrs: I,
        _protocol_type: ProtocolType,
        _address_type: AddressType,
    ) -> Option<SocketAddress> {
        //xxx
        None
    }

    xxx split this routine up into helper routines that can be used by different protocols too.

    pub async fn update_udpv4_dialinfo(
        &self,
        context: &mut DiscoveryContext,
    ) -> Result<(), String> {
        log_net!("looking for udpv4 public dial info");
        let routing_table = self.routing_table();

        let mut retry_count = {
            let c = self.config.get();
            c.network.restricted_nat_retries
        };

        // Get our interface addresses
        let intf_addrs = self.get_local_addresses(ProtocolType::UDP, AddressType::IPV4);

        // Loop for restricted NAT retries
        loop {
            // Get our external address from some fast node, call it node B
            let (external1, node_b) = match self
                .discover_external_address(ProtocolType::UDP, AddressType::IPV4, None)
                .await
            {
                None => {
                    // If we can't get an external address, exit but don't throw an error so we can try again later
                    return Ok(());
                }
                Some(v) => v,
            };
            let external1_dial_info = DialInfo::udp(external1);

            // If our local interface list contains external1 then there is no NAT in place
            if intf_addrs.contains(&external1) {
                // No NAT
                // Do a validate_dial_info on the external address from a redirected node
                if self
                    .validate_dial_info(node_b.clone(), external1_dial_info.clone(), true, false)
                    .await
                {
                    // Add public dial info with Direct dialinfo class
                    routing_table.register_dial_info(
                        RoutingDomain::PublicInternet,
                        external1_dial_info,
                        DialInfoClass::Direct,
                    );
                }
                // Attempt a UDP port mapping via all available and enabled mechanisms
                else if let Some(external_mapped) = self
                    .try_port_mapping(&intf_addrs, ProtocolType::UDP, AddressType::IPV4)
                    .await
                {
                    // Got a port mapping, let's use it
                    let external_mapped_dial_info = DialInfo::udp(external_mapped);
                    routing_table.register_dial_info(
                        RoutingDomain::PublicInternet,
                        external_mapped_dial_info,
                        DialInfoClass::Mapped,
                    );
                } else {
                    // Add public dial info with Blocked dialinfo class
                    routing_table.register_dial_info(
                        RoutingDomain::PublicInternet,
                        external1_dial_info,
                        DialInfoClass::Blocked,
                    );
                }
                context.upgrade_network_class(NetworkClass::InboundCapable);
                // No more retries
                break;
            } else {
                // There is -some NAT-
                // Attempt a UDP port mapping via all available and enabled mechanisms
                if let Some(external_mapped) = self
                    .try_port_mapping(&intf_addrs, ProtocolType::UDP, AddressType::IPV4)
                    .await
                {
                    // Got a port mapping, let's use it
                    let external_mapped_dial_info = DialInfo::udp(external_mapped);
                    routing_table.register_dial_info(
                        RoutingDomain::PublicInternet,
                        external_mapped_dial_info,
                        DialInfoClass::Mapped,
                    );
                    context.upgrade_network_class(NetworkClass::InboundCapable);

                    // No more retries
                    break;
                } else {
                    // Port mapping was not possible, let's see what kind of NAT we have

                    // Does a redirected dial info validation find us?
                    if self
                        .validate_dial_info(
                            node_b.clone(),
                            external1_dial_info.clone(),
                            true,
                            false,
                        )
                        .await
                    {
                        // Yes, another machine can use the dial info directly, so Full Cone
                        // Add public dial info with full cone NAT network class
                        routing_table.register_dial_info(
                            RoutingDomain::PublicInternet,
                            external1_dial_info,
                            DialInfoClass::FullConeNAT,
                        );
                        context.upgrade_network_class(NetworkClass::InboundCapable);

                        // No more retries
                        break;
                    } else {
                        // No, we are restricted, determine what kind of restriction

                        // Get our external address from some fast node, that is not node B, call it node D
                        let (external2, node_d) = match self
                            .discover_external_address(
                                ProtocolType::UDP,
                                AddressType::IPV4,
                                Some(node_b.node_id()),
                            )
                            .await
                        {
                            None => {
                                // If we can't get an external address, exit but don't throw an error so we can try again later
                                return Ok(());
                            }
                            Some(v) => v,
                        };
                        // If we have two different external addresses, then this is a symmetric NAT
                        if external2 != external1 {
                            // Symmetric NAT is outbound only, no public dial info will work
                            context.upgrade_network_class(NetworkClass::OutboundOnly);

                            // No more retries
                            break;
                        } else {
                            // If we're going to end up as a restricted NAT of some sort
                            // we should go through our retries before we assign a dial info
                            if retry_count == 0 {
                                // Address is the same, so it's address or port restricted
                                let external2_dial_info = DialInfo::udp(external2);
                                // Do a validate_dial_info on the external address from a routed node
                                if self
                                    .validate_dial_info(
                                        node_d.clone(),
                                        external2_dial_info.clone(),
                                        false,
                                        true,
                                    )
                                    .await
                                {
                                    // Got a reply from a non-default port, which means we're only address restricted
                                    routing_table.register_dial_info(
                                        RoutingDomain::PublicInternet,
                                        external1_dial_info,
                                        DialInfoClass::AddressRestrictedNAT,
                                    );
                                } else {
                                    // Didn't get a reply from a non-default port, which means we are also port restricted
                                    routing_table.register_dial_info(
                                        RoutingDomain::PublicInternet,
                                        external1_dial_info,
                                        DialInfoClass::PortRestrictedNAT,
                                    );
                                }
                                context.upgrade_network_class(NetworkClass::InboundCapable);
                            }
                        }
                    }
                }

                if retry_count == 0 {
                    break;
                }
                retry_count -= 1;
            }
        }

        // xxx should verify hole punch capable somehow and switch to outbound-only if hole punch can't work

        Ok(())
    }

    pub async fn update_tcpv4_dialinfo(
        &self,
        context: &mut DiscoveryContext,
    ) -> Result<(), String> {
        log_net!("looking for tcpv4 public dial info");

        Ok(())
    }

    pub async fn update_wsv4_dialinfo(&self, context: &mut DiscoveryContext) -> Result<(), String> {
        log_net!("looking for wsv4 public dial info");
        // xxx
        //Err("unimplemented".to_owned())
        Ok(())
    }

    pub async fn update_udpv6_dialinfo(
        &self,
        context: &mut DiscoveryContext,
    ) -> Result<(), String> {
        log_net!("looking for udpv6 public dial info");
        // xxx
        //Err("unimplemented".to_owned())
        Ok(())
    }

    pub async fn update_tcpv6_dialinfo(
        &self,
        context: &mut DiscoveryContext,
    ) -> Result<(), String> {
        log_net!("looking for tcpv6 public dial info");
        // xxx
        //Err("unimplemented".to_owned())
        Ok(())
    }

    pub async fn update_wsv6_dialinfo(&self, context: &mut DiscoveryContext) -> Result<(), String> {
        log_net!("looking for wsv6 public dial info");
        // xxx
        //Err("unimplemented".to_owned())
        Ok(())
    }

    pub async fn update_network_class_task_routine(self, _l: u64, _t: u64) -> Result<(), String> {
        log_net!("updating network class");

        let protocol_config = self
            .inner
            .lock()
            .protocol_config
            .clone()
            .unwrap_or_default();

        let mut context = DiscoveryContext::default();

        if protocol_config.inbound.contains(ProtocolType::UDP) {
            self.update_udpv4_dialinfo(&mut context).await?;
            self.update_udpv6_dialinfo(&mut context).await?;
        }

        if protocol_config.inbound.contains(ProtocolType::TCP) {
            self.update_tcpv4_dialinfo(&mut context).await?;
            self.update_tcpv6_dialinfo(&mut context).await?;
        }

        if protocol_config.inbound.contains(ProtocolType::WS) {
            self.update_wsv4_dialinfo(&mut context).await?;
            self.update_wsv6_dialinfo(&mut context).await?;
        }

        self.inner.lock().network_class = context.network_class;

        Ok(())
    }
}
