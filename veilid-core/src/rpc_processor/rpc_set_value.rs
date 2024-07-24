use super::*;

#[derive(Clone, Debug)]
pub struct SetValueAnswer {
    pub set: bool,
    pub value: Option<SignedValueData>,
    pub peers: PeerInfoResponse,
}

impl RPCProcessor {
    /// Sends a set value request and wait for response
    /// Can be sent via all methods including relays
    /// Safety routes may be used, but never private routes.
    /// Because this leaks information about the identity of the node itself,
    /// replying to this request received over a private route will leak
    /// the identity of the node and defeat the private route.
    #[instrument(level = "trace", target = "rpc", skip(self, value, descriptor), 
        fields(value.data.len = value.value_data().data().len(), 
            value.data.seq = value.value_data().seq(), 
            value.data.writer = value.value_data().writer().to_string(), 
            ret.set,
            ret.value.data.len, 
            ret.value.data.seq, 
            ret.value.data.writer, 
            ret.peers.len,
            ret.latency
        ), err)]
    pub async fn rpc_call_set_value(
        self,
        dest: Destination,
        key: TypedKey,
        subkey: ValueSubkey,
        value: SignedValueData,
        descriptor: SignedValueDescriptor,
        send_descriptor: bool,
    ) ->RPCNetworkResult<Answer<SetValueAnswer>> {
        let _guard = self
            .unlocked_inner
            .startup_lock
            .enter()
            .map_err(RPCError::map_try_again("not started up"))?;

        // Ensure destination never has a private route
        // and get the target noderef so we can validate the response
        let Some(target) = dest.node() else {
            return Err(RPCError::internal(
                "Never send set value requests over private routes",
            ));
        };

        // Get the target node id
        let Some(vcrypto) = self.crypto.get(key.kind) else {
            return Err(RPCError::internal("unsupported cryptosystem"));
        };
        let Some(target_node_id) = target.node_ids().get(key.kind) else {
            return Err(RPCError::internal("No node id for crypto kind"));
        };

        let debug_string = format!(
            "OUT ==> SetValueQ({} #{} len={} writer={}{}) => {}",
            key,
            subkey,
            value.value_data().data().len(),
            value.value_data().writer(),
            if send_descriptor {
                " +senddesc"
            } else {
                ""
            },
            dest
        );

        // Send the setvalue question
        let set_value_q = RPCOperationSetValueQ::new(
            key,
            subkey,
            value,
            if send_descriptor {
                Some(descriptor.clone())
            } else {
                None
            },
        );
        let question = RPCQuestion::new(
            network_result_try!(self.get_destination_respond_to(&dest)?),
            RPCQuestionDetail::SetValueQ(Box::new(set_value_q)),
        );
        let question_context = QuestionContext::SetValue(ValidateSetValueContext {
            descriptor,
            subkey,
            vcrypto: vcrypto.clone(),
        });

        if debug_target_enabled!("dht") {
            log_dht!(debug "{}", debug_string);
        }

        let safety_domain_set = if dest.has_safety_route() {
            SafetyDomain::Safe.into()
        } else {
            SafetyDomainSet::all()
        };

        let waitable_reply = network_result_try!(
            self.question(dest.clone(), question, Some(question_context))
                .await?
        );

        // Keep the reply private route that was used to return with the answer
        let reply_private_route = waitable_reply.reply_private_route;

        // Wait for reply
        let (msg, latency) = match self.wait_for_reply(waitable_reply, debug_string).await? {
            TimeoutOr::Timeout => return Ok(NetworkResult::Timeout),
            TimeoutOr::Value(v) => v,
        };

        // Get the right answer type
        let (_, _, _, kind) = msg.operation.destructure();
        let set_value_a = match kind {
            RPCOperationKind::Answer(a) => match a.destructure() {
                RPCAnswerDetail::SetValueA(a) => a,
                _ => return Ok(NetworkResult::invalid_message("not a setvalue answer")),
            },
            _ => return Ok(NetworkResult::invalid_message("not an answer")),
        };

        let (set, value, peer_info_list) = set_value_a.destructure();

        if debug_target_enabled!("dht") {
            let debug_string_value = value.as_ref().map(|v| {
                format!(" len={} writer={}",
                    v.value_data().data().len(),
                    v.value_data().writer(),
                )
            }).unwrap_or_default();
            

            let debug_string_answer = format!(
                "OUT <== SetValueA({} #{}{}{} peers={}) <= {}",
                key,
                subkey,
                if set {
                    " +set"
                } else {
                    ""
                },
                debug_string_value,
                peer_info_list.len(),
                dest,
            );

            log_dht!(debug "{}", debug_string_answer);
            
            let peer_ids:Vec<String> = peer_info_list.iter().filter_map(|p| p.node_ids().get(key.kind).map(|k| k.to_string())).collect();
            log_dht!(debug "Peers: {:#?}", peer_ids);
        }

        // Validate peers returned are, in fact, closer to the key than the node we sent this to
        let valid = match RoutingTable::verify_peers_closer(vcrypto, target_node_id, key, &peer_info_list) {
            Ok(v) => v,
            Err(e) => {
                return Ok(NetworkResult::invalid_message(format!(
                    "missing cryptosystem in peers node ids: {}",
                    e
                )));
            }
        };
        if !valid {
            return Ok(NetworkResult::invalid_message("non-closer peers returned"));
        }

        #[cfg(feature = "verbose-tracing")]
        tracing::Span::current().record("ret.latency", latency.as_u64());
        #[cfg(feature = "verbose-tracing")]
        tracing::Span::current().record("ret.set", set);
        #[cfg(feature = "verbose-tracing")]
        if let Some(value) = &value {
            tracing::Span::current().record("ret.value.data.len", value.value_data().data().len());
            tracing::Span::current().record("ret.value.data.seq", value.value_data().seq());
            tracing::Span::current().record("ret.value.data.writer", value.value_data().writer().to_string());
        }
        #[cfg(feature = "verbose-tracing")]
        tracing::Span::current().record("ret.peers.len", peers.len());

        Ok(NetworkResult::value(Answer::new(
            latency,
            reply_private_route,
            SetValueAnswer { set, value, peers: PeerInfoResponse { safety_domain_set, peer_info_list } }
        )))
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////

    #[instrument(level = "trace", target = "rpc", skip(self, msg), fields(msg.operation.op_id), ret, err)]
    pub(crate) async fn process_set_value_q(
        &self,
        msg: RPCMessage,
    ) ->RPCNetworkResult<()> {
        // Ignore if disabled
        let routing_table = self.routing_table();
        let rss = routing_table.route_spec_store();
        
        let opi = routing_table.get_own_peer_info(msg.header.routing_domain());
        if !opi
            .signed_node_info()
            .node_info()
            .has_capability(CAP_DHT)
        {
            return Ok(NetworkResult::service_unavailable(
                "dht is not available",
            ));
        }
        
        // Ensure this never came over a private route, safety route is okay though
        match &msg.header.detail {
            RPCMessageHeaderDetail::Direct(_) | RPCMessageHeaderDetail::SafetyRouted(_) => {}
            RPCMessageHeaderDetail::PrivateRouted(_) => {
                return Ok(NetworkResult::invalid_message(
                    "not processing set value request over private route",
                ))
            }
        }

        // Get the question
        let kind = msg.operation.kind().clone();
        let set_value_q = match kind {
            RPCOperationKind::Question(q) => match q.destructure() {
                (_, RPCQuestionDetail::SetValueQ(q)) => q,
                _ => panic!("not a setvalue question"),
            },
            _ => panic!("not a question"),
        };

        // Destructure
        let (key, subkey, value, descriptor) = set_value_q.destructure();

        // Get target for ValueChanged notifications
        let dest = network_result_try!(self.get_respond_to_destination(&msg));
        let target = dest.get_target(rss)?;
        
        // Get the nodes that we know about that are closer to the the key than our own node
        let routing_table = self.routing_table();
        let closer_to_key_peers = network_result_try!(routing_table.find_preferred_peers_closer_to_key(key, vec![CAP_DHT]));

        let debug_string = format!(
            "IN <=== SetValueQ({} #{} len={} seq={} writer={}{}) <== {}",
            key,
            subkey,
            value.value_data().data().len(),
            value.value_data().seq(),
            value.value_data().writer(),
            if descriptor.is_some() {
                " +desc"
            } else {
                ""
            },
            msg.header.direct_sender_node_id()
        );

        log_rpc!(debug "{}", debug_string);

        // If there are less than 'set_value_count' peers that are closer, then store here too
        let set_value_count = {
            let c = self.config.get();
            c.network.dht.set_value_count as usize
        };
        let (set, new_value) = if closer_to_key_peers.len() >= set_value_count {
            // Not close enough
            (false, None)
        } else {
            // Close enough, lets set it

            // Save the subkey, creating a new record if necessary
            let storage_manager = self.storage_manager();
            let new_value = network_result_try!(storage_manager
                .inbound_set_value(key, subkey, Arc::new(value), descriptor.map(Arc::new), target)
                .await
                .map_err(RPCError::internal)?);

            (true, new_value)
        };

        if debug_target_enabled!("dht") {
            let debug_string_value = new_value.as_ref().map(|v| {
                format!(" len={} seq={} writer={}",
                    v.value_data().data().len(),
                    v.value_data().seq(),
                    v.value_data().writer(),
                )
            }).unwrap_or_default();

            let debug_string_answer = format!(
                "IN ===> SetValueA({} #{}{}{} peers={}) ==> {}",
                key,
                subkey,
                if set {
                    " +set"
                } else {
                    ""
                },
                debug_string_value,
                closer_to_key_peers.len(),
                msg.header.direct_sender_node_id()
            );

            log_dht!(debug "{}", debug_string_answer);
        }

        // Make SetValue answer
        let set_value_a = RPCOperationSetValueA::new(set, new_value.map(|x| (*x).clone()), closer_to_key_peers)?;

        // Send SetValue answer
        self.answer(msg, RPCAnswer::new(RPCAnswerDetail::SetValueA(Box::new(set_value_a))))
            .await
    }
}
