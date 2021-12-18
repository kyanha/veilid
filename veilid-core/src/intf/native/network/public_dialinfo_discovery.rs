use super::*;

use crate::intf::*;
use crate::network_manager::*;
use crate::routing_table::*;
use crate::*;

use async_std::net::*;

impl Network {
    // Ask for a public address check from a particular noderef
    async fn request_public_address(&self, node_ref: NodeRef) -> Option<SocketAddr> {
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
        protocol_address_type: ProtocolAddressType,
        ignore_node: Option<DHTKey>,
    ) -> Option<(SocketAddr, NodeRef)> {
        let routing_table = self.routing_table();
        let peers = routing_table.get_fast_nodes_of_type(protocol_address_type);
        if peers.is_empty() {
            log_net!("no peers of type '{:?}'", protocol_address_type);
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

    fn get_interface_addresses(
        &self,
        protocol_address_type: ProtocolAddressType,
    ) -> Vec<SocketAddr> {
        let routing_table = self.routing_table();

        routing_table
            .get_own_peer_info(PeerScope::Local)
            .dial_infos
            .iter()
            .filter_map(|di| {
                if di.protocol_address_type() == protocol_address_type {
                    if let Ok(addr) = di.to_socket_addr() {
                        return Some(addr);
                    }
                }
                None
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

    async fn try_port_mapping<I: AsRef<[SocketAddr]>>(
        &self,
        _intf_addrs: I,
        _protocol_address_type: ProtocolAddressType,
    ) -> Option<SocketAddr> {
        //xxx
        None
    }

    pub async fn update_udpv4_dialinfo_task_routine(self, _l: u64, _t: u64) -> Result<(), String> {
        log_net!("looking for udpv4 public dial info");
        let routing_table = self.routing_table();

        let mut retry_count = {
            let c = self.config.get();
            c.network.restricted_nat_retries
        };

        // Get our interface addresses
        let intf_addrs = self.get_interface_addresses(ProtocolAddressType::UDPv4);

        // Loop for restricted NAT retries
        loop {
            // Get our external address from some fast node, call it node B
            let (external1, node_b) = match self
                .discover_external_address(ProtocolAddressType::UDPv4, None)
                .await
            {
                None => {
                    // If we can't get an external address, exit but don't throw an error so we can try again later
                    return Ok(());
                }
                Some(v) => v,
            };
            let external1_dial_info = DialInfo::udp_from_socketaddr(external1);

            // If our local interface list contains external1 then there is no NAT in place
            if intf_addrs.contains(&external1) {
                // No NAT
                // Do a validate_dial_info on the external address from a routed node
                if self
                    .validate_dial_info(node_b.clone(), external1_dial_info.clone(), true, false)
                    .await
                {
                    // Add public dial info with Server network class
                    routing_table.register_global_dial_info(
                        external1_dial_info,
                        Some(NetworkClass::Server),
                        DialInfoOrigin::Discovered,
                    );

                    // No more retries
                    break;
                } else {
                    // UDP firewall?
                    log_net!("UDP static public dial info not reachable. UDP firewall may be blocking inbound to {:?} for {:?}",external1_dial_info, node_b);
                }
            } else {
                // There is -some NAT-
                // Attempt a UDP port mapping via all available and enabled mechanisms
                if let Some(external_mapped) = self
                    .try_port_mapping(&intf_addrs, ProtocolAddressType::UDPv4)
                    .await
                {
                    // Got a port mapping, let's use it
                    let external_mapped_dial_info = DialInfo::udp_from_socketaddr(external_mapped);
                    routing_table.register_global_dial_info(
                        external_mapped_dial_info,
                        Some(NetworkClass::Mapped),
                        DialInfoOrigin::Mapped,
                    );

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
                        routing_table.register_global_dial_info(
                            external1_dial_info,
                            Some(NetworkClass::FullNAT),
                            DialInfoOrigin::Discovered,
                        );

                        // No more retries
                        break;
                    } else {
                        // No, we are restricted, determine what kind of restriction

                        // Get our external address from some fast node, that is not node B, call it node D
                        let (external2, node_d) = match self
                            .discover_external_address(
                                ProtocolAddressType::UDPv4,
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
                            self.inner.lock().network_class = Some(NetworkClass::OutboundOnly);

                            // No more retries
                            break;
                        } else {
                            // If we're going to end up as a restricted NAT of some sort
                            // we should go through our retries before we assign a dial info
                            if retry_count == 0 {
                                // Address is the same, so it's address or port restricted
                                let external2_dial_info = DialInfo::udp_from_socketaddr(external2);
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
                                    routing_table.register_global_dial_info(
                                        external1_dial_info,
                                        Some(NetworkClass::AddressRestrictedNAT),
                                        DialInfoOrigin::Discovered,
                                    );
                                } else {
                                    // Didn't get a reply from a non-default port, which means we are also port restricted
                                    routing_table.register_global_dial_info(
                                        external1_dial_info,
                                        Some(NetworkClass::PortRestrictedNAT),
                                        DialInfoOrigin::Discovered,
                                    );
                                }
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

        Ok(())
    }

    pub async fn update_tcpv4_dialinfo_task_routine(self, _l: u64, _t: u64) -> Result<(), String> {
        log_net!("looking for tcpv4 public dial info");
        // xxx
        //Err("unimplemented".to_owned())
        Ok(())
    }
}
