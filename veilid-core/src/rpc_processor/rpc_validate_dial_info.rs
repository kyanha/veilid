use super::*;

impl RPCProcessor {
    // Can only be sent directly, not via relays or routes
    pub async fn rpc_call_validate_dial_info(
        self,
        peer: NodeRef,
        dial_info: DialInfo,
        redirect: bool,
    ) -> Result<bool, RPCError> {
        let network_manager = self.network_manager();
        let receipt_time = ms_to_us(
            self.config
                .get()
                .network
                .dht
                .validate_dial_info_receipt_time_ms,
        );

        // Generate receipt and waitable eventual so we can see if we get the receipt back
        let (receipt, eventual_value) = network_manager
            .generate_single_shot_receipt(receipt_time, [])
            .map_err(RPCError::internal)?;

        let validate_dial_info = RPCOperationValidateDialInfo {
            dial_info,
            receipt,
            redirect,
        };
        let statement = RPCStatement::new(RPCStatementDetail::ValidateDialInfo(validate_dial_info));

        // Send the validate_dial_info request
        // This can only be sent directly, as relays can not validate dial info
        self.statement(Destination::Direct(peer), statement, None)
            .await?;

        // Wait for receipt
        match eventual_value.await.take_value().unwrap() {
            ReceiptEvent::ReturnedInBand { inbound_noderef: _ } => Err(RPCError::internal(
                "validate_dial_info receipt should be returned out-of-band",
            )),
            ReceiptEvent::ReturnedOutOfBand => {
                log_net!(debug "validate_dial_info receipt returned");
                Ok(true)
            }
            ReceiptEvent::Expired => {
                log_net!(debug "validate_dial_info receipt expired");
                Ok(false)
            }
            ReceiptEvent::Cancelled => {
                Err(RPCError::internal("receipt was dropped before expiration"))
            }
        }
    }

    pub(crate) async fn process_validate_dial_info(&self, msg: RPCMessage) -> Result<(), RPCError> {
        // Get the statement
        let RPCOperationValidateDialInfo {
            dial_info,
            receipt,
            redirect,
        } = match msg.operation.into_kind() {
            RPCOperationKind::Statement(s) => match s.into_detail() {
                RPCStatementDetail::ValidateDialInfo(s) => s,
                _ => panic!("not a validate dial info"),
            },
            _ => panic!("not a statement"),
        };

        // Redirect this request if we are asked to
        if redirect {
            // Find peers capable of validating this dial info
            // We filter on the -outgoing- protocol capability status not the node's dial info
            // Use the address type though, to ensure we reach an ipv6 capable node if this is
            // an ipv6 address
            let routing_table = self.routing_table();
            let filter = DialInfoFilter::global().with_address_type(dial_info.address_type());
            let sender_id = msg.header.envelope.get_sender_id();
            let node_count = {
                let c = self.config.get();
                c.network.dht.max_find_node_count as usize
            };
            let peers = routing_table.find_fast_public_nodes_filtered(node_count, &filter);
            if peers.is_empty() {
                return Err(RPCError::internal(format!(
                    "no peers matching filter '{:?}'",
                    filter
                )));
            }
            for mut peer in peers {
                // Ensure the peer is not the one asking for the validation
                if peer.node_id() == sender_id {
                    continue;
                }

                // Release the filter on the peer because we don't need to send the redirect with the filter
                // we just wanted to make sure we only selected nodes that were capable of
                // using the correct protocol for the dial info being validated
                peer.set_filter(None);

                // Ensure the peer's status is known and that it is capable of
                // making outbound connections for the dial info we want to verify
                // and if this peer can validate dial info
                let can_contact_dial_info = peer.operate(|e: &BucketEntryInner| {
                    if let Some(ni) = e.node_info() {
                        ni.outbound_protocols.contains(dial_info.protocol_type())
                            && ni.can_validate_dial_info()
                    } else {
                        false
                    }
                });
                if !can_contact_dial_info {
                    continue;
                }

                // See if this peer will validate dial info
                let will_validate_dial_info = peer.operate(|e: &BucketEntryInner| {
                    if let Some(status) = &e.peer_stats().status {
                        status.will_validate_dial_info
                    } else {
                        true
                    }
                });
                if !will_validate_dial_info {
                    continue;
                }

                // Make a copy of the request, without the redirect flag
                let validate_dial_info = RPCOperationValidateDialInfo {
                    dial_info: dial_info.clone(),
                    receipt: receipt.clone(),
                    redirect: false,
                };
                let statement =
                    RPCStatement::new(RPCStatementDetail::ValidateDialInfo(validate_dial_info));

                // Send the validate_dial_info request
                // This can only be sent directly, as relays can not validate dial info
                self.statement(Destination::Direct(peer), statement, None)
                    .await?;
            }
            return Ok(());
        };

        // Otherwise send a return receipt directly
        // Possibly from an alternate port
        let network_manager = self.network_manager();
        network_manager
            .send_out_of_band_receipt(dial_info.clone(), receipt)
            .await
            .map_err(RPCError::network)?;

        Ok(())
    }
}
