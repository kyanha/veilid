use super::*;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Default)]
pub struct SenderInfo {
    pub socket_address: SocketAddress,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Default)]
pub struct StatusResult {
    pub opt_sender_info: Option<SenderInfo>,
    pub opt_previous_sender_info: Option<SenderInfo>,
}

impl RPCProcessor {
    // Send StatusQ RPC request, receive StatusA answer
    // Can be sent via relays or routes, but will have less information via routes
    // sender:
    // unsafe -> node status
    // safe -> nothing
    // receiver:
    // direct -> node status + sender info
    // safety -> node status
    // private -> nothing
    #[instrument(level = "trace", target = "rpc", skip(self), ret, err)]
    pub async fn rpc_call_status(
        self,
        dest: Destination,
    ) -> RPCNetworkResult<Answer<StatusResult>> {
        let _guard = self
            .unlocked_inner
            .startup_lock
            .enter()
            .map_err(RPCError::map_try_again("not started up"))?;

        // Determine routing domain and node status to send
        let (opt_target_nr, routing_domain, node_status) = if let Some(UnsafeRoutingInfo {
            opt_node,
            opt_relay,
            opt_routing_domain,
        }) =
            dest.get_unsafe_routing_info(self.routing_table.clone())
        {
            let Some(routing_domain) = opt_routing_domain else {
                // Because this exits before calling 'question()',
                // a failure to find a routing domain constitutes a send failure
                // Record the send failure on both the node and its relay
                let send_ts = Timestamp::now();
                if let Some(node) = &opt_node {
                    self.record_send_failure(RPCKind::Question, send_ts, node.clone(), None, None);
                }
                if let Some(relay) = &opt_relay {
                    self.record_send_failure(RPCKind::Question, send_ts, relay.clone(), None, None);
                }
                return Ok(NetworkResult::no_connection_other(
                    "no routing domain for target",
                ));
            };

            let node_status = Some(self.network_manager().generate_node_status(routing_domain));
            (opt_node, routing_domain, node_status)
        } else {
            // Safety route means we don't exchange node status and things are all PublicInternet RoutingDomain
            (None, RoutingDomain::PublicInternet, None)
        };

        // Create status rpc question
        let status_q = RPCOperationStatusQ::new(node_status);
        let question = RPCQuestion::new(
            network_result_try!(self.get_destination_respond_to(&dest)?),
            RPCQuestionDetail::StatusQ(Box::new(status_q)),
        );

        let debug_string = format!("Status => {}", dest);

        // Send the info request
        let waitable_reply =
            network_result_try!(self.question(dest.clone(), question, None).await?);

        // Note what kind of ping this was and to what peer scope
        let send_data_method = waitable_reply.send_data_method.clone();

        // Keep the reply private route that was used to return with the answer
        let reply_private_route = waitable_reply.reply_private_route;

        // Wait for reply
        let (msg, latency) = match self.wait_for_reply(waitable_reply, debug_string).await? {
            TimeoutOr::Timeout => return Ok(NetworkResult::Timeout),
            TimeoutOr::Value(v) => v,
        };

        // Get the right answer type
        let (_, _, kind) = msg.operation.destructure();
        let status_a = match kind {
            RPCOperationKind::Answer(a) => match a.destructure() {
                RPCAnswerDetail::StatusA(a) => a,
                _ => return Ok(NetworkResult::invalid_message("not a status answer")),
            },
            _ => return Ok(NetworkResult::invalid_message("not an answer")),
        };
        let (a_node_status, sender_info) = status_a.destructure();

        // Ensure the returned node status is the kind for the routing domain we asked for
        if let Some(target_nr) = opt_target_nr {
            if let Some(a_node_status) = a_node_status {
                // Update latest node status in routing table
                target_nr.update_node_status(routing_domain, a_node_status.clone());
            }
        }

        // Report sender_info IP addresses to network manager
        // Don't need to validate these addresses for the current routing domain
        // the address itself is irrelevant, and the remote node can lie anyway
        let mut opt_sender_info = None;
        let mut opt_previous_sender_info = None;
        match dest {
            Destination::Direct {
                node: target,
                safety_selection,
            } => {
                if matches!(safety_selection, SafetySelection::Unsafe(_)) {
                    if let Some(sender_info) = sender_info {
                        if send_data_method.opt_relayed_contact_method.is_none()
                            && matches!(
                                send_data_method.contact_method,
                                NodeContactMethod::Direct(_)
                            )
                        {
                            // Directly requested status that actually gets sent directly and not over a relay will tell us what our IP address appears as
                            // If this changes, we'd want to know about that to reset the networking stack
                            opt_previous_sender_info = target.report_sender_info(
                                routing_domain,
                                send_data_method.unique_flow.flow.protocol_type(),
                                send_data_method.unique_flow.flow.address_type(),
                                sender_info,
                            );
                        };
                        opt_sender_info = Some(sender_info);

                        // Report ping status results to network manager
                        self.network_manager().report_socket_address_change(
                            routing_domain,
                            sender_info.socket_address,
                            opt_previous_sender_info.map(|s| s.socket_address),
                            send_data_method.unique_flow.flow,
                            target.unfiltered(),
                        );
                    }
                }
            }
            Destination::Relay {
                relay: _,
                node: _,
                safety_selection: _,
            }
            | Destination::PrivateRoute {
                private_route: _,
                safety_selection: _,
            } => {
                // sender info is irrelevant over relays and routes
            }
        };
        Ok(NetworkResult::value(Answer::new(
            latency,
            reply_private_route,
            StatusResult {
                opt_sender_info,
                opt_previous_sender_info,
            },
        )))
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////

    #[instrument(level = "trace", target = "rpc", skip(self, msg), fields(msg.operation.op_id), ret, err)]
    pub(crate) async fn process_status_q(&self, msg: RPCMessage) -> RPCNetworkResult<()> {
        // Get the question
        let kind = msg.operation.kind().clone();
        let status_q = match kind {
            RPCOperationKind::Question(q) => match q.destructure() {
                (_, RPCQuestionDetail::StatusQ(q)) => q,
                _ => panic!("not a status question"),
            },
            _ => panic!("not a question"),
        };
        let q_node_status = status_q.destructure();

        let (node_status, sender_info) = match &msg.header.detail {
            RPCMessageHeaderDetail::Direct(detail) => {
                let flow = detail.flow;
                let routing_domain = detail.routing_domain;

                // Ensure the node status from the question is the kind for the routing domain we received the request in
                if let Some(q_node_status) = q_node_status {
                    // update node status for the requesting node to our routing table
                    if let Some(sender_nr) = msg.opt_sender_nr.clone() {
                        // Update latest node status in routing table for the statusq sender
                        sender_nr.update_node_status(routing_domain, q_node_status.clone());
                    }
                }

                // Get the peer address in the returned sender info
                let sender_info = SenderInfo {
                    socket_address: *flow.remote_address(),
                };

                // Make status answer
                let node_status = self.network_manager().generate_node_status(routing_domain);
                (Some(node_status), Some(sender_info))
            }
            RPCMessageHeaderDetail::SafetyRouted(_) => {
                // Make status answer
                let node_status = self
                    .network_manager()
                    .generate_node_status(RoutingDomain::PublicInternet);
                (Some(node_status), None)
            }
            RPCMessageHeaderDetail::PrivateRouted(_) => (None, None),
        };

        // Make status answer
        let status_a = RPCOperationStatusA::new(node_status, sender_info);

        // Send status answer
        self.answer(
            msg,
            RPCAnswer::new(RPCAnswerDetail::StatusA(Box::new(status_a))),
        )
        .await
    }
}
