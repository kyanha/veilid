use super::*;

impl RPCProcessor {
    // Can only be sent directly, not via relays or routes
    pub async fn rpc_call_validate_dial_info(
        &self,
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
        //
        let (vdi_msg, eventual_value) = {
            let mut vdi_msg = ::capnp::message::Builder::new_default();
            let mut question = vdi_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let mut vdi_builder = detail.init_validate_dial_info();

            // Generate receipt and waitable eventual so we can see if we get the receipt back
            let (receipt, eventual_value) = network_manager
                .generate_single_shot_receipt(receipt_time, [])
                .map_err(map_error_string!())?;

            vdi_builder.set_redirect(redirect);
            let mut di_builder = vdi_builder.reborrow().init_dial_info();
            encode_dial_info(&dial_info, &mut di_builder)?;
            let r_builder = vdi_builder.init_receipt(receipt.len().try_into().map_err(
                map_error_protocol!("invalid receipt length in validate dial info"),
            )?);
            r_builder.copy_from_slice(&receipt);

            (vdi_msg.into_reader(), eventual_value)
        };

        // Send the validate_dial_info request
        // This can only be sent directly, as relays can not validate dial info
        self.request(Destination::Direct(peer), vdi_msg, None)
            .await?;

        log_net!(debug "waiting for validate_dial_info receipt");
        // Wait for receipt
        match eventual_value.await.take_value().unwrap() {
            ReceiptEvent::ReturnedInBand { inbound_noderef: _ } => Err(rpc_error_internal(
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
                Err(rpc_error_internal("receipt was dropped before expiration"))
            }
        }
    }

    pub(crate) async fn process_validate_dial_info(
        &self,
        rpcreader: RPCMessage,
    ) -> Result<(), RPCError> {
        //
        let (redirect, dial_info, receipt) = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(rpc_error_invalid_format(
                    "validate dial info should not want answer",
                ));
            }

            // get validateDialInfo reader
            let vdi_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::ValidateDialInfo(Ok(x))) => x,
                _ => panic!("invalid operation type in process_validate_dial_info"),
            };

            // Parse out fields
            let redirect = vdi_reader.get_redirect();
            let dial_info = decode_dial_info(&vdi_reader.get_dial_info().map_err(
                map_error_internal!("no valid dial info in process_validate_dial_info"),
            )?)?;
            let receipt = vdi_reader
                .get_receipt()
                .map_err(map_error_internal!(
                    "no valid receipt in process_validate_dial_info"
                ))?
                .to_vec();

            (redirect, dial_info, receipt)
        };

        // Redirect this request if we are asked to
        if redirect {
            // Find peers capable of validating this dial info
            // We filter on the -outgoing- protocol capability status not the node's dial info
            // Use the address type though, to ensure we reach an ipv6 capable node if this is
            // an ipv6 address
            let routing_table = self.routing_table();
            let filter = DialInfoFilter::global().with_address_type(dial_info.address_type());
            let sender_id = rpcreader.header.envelope.get_sender_id();
            let node_count = {
                let c = self.config.get();
                c.network.dht.max_find_node_count as usize
            };
            let mut peers = routing_table.find_fast_public_nodes_filtered(node_count, &filter);
            if peers.is_empty() {
                return Err(rpc_error_internal(format!(
                    "no peers matching filter '{:?}'",
                    filter
                )));
            }
            for peer in &mut peers {
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
                let vdi_msg_reader = {
                    let mut vdi_msg = ::capnp::message::Builder::new_default();
                    let mut question = vdi_msg.init_root::<veilid_capnp::operation::Builder>();
                    question.set_op_id(self.get_next_op_id());
                    let mut respond_to = question.reborrow().init_respond_to();
                    respond_to.set_none(());
                    let detail = question.reborrow().init_detail();
                    let mut vdi_builder = detail.init_validate_dial_info();
                    vdi_builder.set_redirect(false);
                    let mut di_builder = vdi_builder.reborrow().init_dial_info();
                    encode_dial_info(&dial_info, &mut di_builder)?;
                    let r_builder = vdi_builder.init_receipt(receipt.len().try_into().map_err(
                        map_error_protocol!("invalid receipt length in process_validate_dial_info"),
                    )?);
                    r_builder.copy_from_slice(&receipt);
                    vdi_msg.into_reader()
                };

                // Send the validate_dial_info request until we succeed
                self.request(Destination::Direct(peer.clone()), vdi_msg_reader, None)
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
            .map_err(map_error_string!())
            .map_err(
                logthru_net!(error "failed to send direct receipt to dial info: {}", dial_info),
            )?;

        Ok(())
    }
}
