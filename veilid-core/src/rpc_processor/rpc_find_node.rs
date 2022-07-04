use super::*;

impl RPCProcessor {
    // Send FindNodeQ RPC request, receive FindNodeA answer
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_find_node(
        self,
        dest: Destination,
        key: DHTKey,
        safety_route: Option<&SafetyRouteSpec>,
        respond_to: RespondTo,
    ) -> Result<FindNodeAnswer, RPCError> {
        let find_node_q_msg = {
            let mut find_node_q_msg = ::capnp::message::Builder::new_default();
            let mut question = find_node_q_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to_builder = question.reborrow().init_respond_to();
            respond_to.encode(&mut respond_to_builder)?;
            let detail = question.reborrow().init_detail();
            let mut fnq = detail.init_find_node_q();
            let mut node_id_builder = fnq.reborrow().init_node_id();
            encode_public_key(&key, &mut node_id_builder)?;

            find_node_q_msg.into_reader()
        };

        // Send the find_node request
        let waitable_reply = self
            .request(dest, find_node_q_msg, safety_route)
            .await?
            .unwrap();

        // Wait for reply
        let (rpcreader, latency) = self.wait_for_reply(waitable_reply).await?;

        let response_operation = rpcreader
            .reader
            .get_root::<veilid_capnp::operation::Reader>()
            .map_err(map_error_capnp_error!())
            .map_err(logthru_rpc!())?;
        let find_node_a = match response_operation
            .get_detail()
            .which()
            .map_err(map_error_capnp_notinschema!())
            .map_err(logthru_rpc!())?
        {
            veilid_capnp::operation::detail::FindNodeA(a) => {
                a.map_err(map_error_internal!("Invalid FindNodeA"))?
            }
            _ => return Err(rpc_error_internal("Incorrect RPC answer for question")),
        };

        let peers_reader = find_node_a
            .get_peers()
            .map_err(map_error_internal!("Missing peers"))?;
        let mut peers = Vec::<PeerInfo>::with_capacity(
            peers_reader
                .len()
                .try_into()
                .map_err(map_error_internal!("too many peers"))?,
        );
        for p in peers_reader.iter() {
            let peer_info = decode_peer_info(&p, true)?;

            if !self.filter_peer_scope(&peer_info.signed_node_info.node_info) {
                return Err(rpc_error_invalid_format(
                    "find_node response has invalid peer scope",
                ));
            }

            peers.push(peer_info);
        }

        let out = FindNodeAnswer { latency, peers };

        Ok(out)
    }

    pub(crate) async fn process_find_node_q(&self, rpcreader: RPCMessage) -> Result<(), RPCError> {
        //
        let reply_msg = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // find_node must always want an answer
            if !self.wants_answer(&operation)? {
                return Err(rpc_error_invalid_format("find_node_q should want answer"));
            }

            // get findNodeQ reader
            let fnq_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::FindNodeQ(Ok(x))) => x,
                _ => panic!("invalid operation type in process_find_node_q"),
            };

            // get the node id we want to look up
            let target_node_id = decode_public_key(
                &fnq_reader
                    .get_node_id()
                    .map_err(map_error_capnp_error!())
                    .map_err(logthru_rpc!())?,
            );

            // add node information for the requesting node to our routing table
            let routing_table = self.routing_table();

            // find N nodes closest to the target node in our routing table
            let own_peer_info = routing_table.get_own_peer_info();
            let own_peer_info_is_valid = own_peer_info.signed_node_info.is_valid();

            let closest_nodes = routing_table.find_closest_nodes(
                target_node_id,
                // filter
                Some(move |_k, v| {
                    RoutingTable::filter_has_valid_signed_node_info(v, own_peer_info_is_valid)
                }),
                // transform
                move |k, v| RoutingTable::transform_to_peer_info(k, v, &own_peer_info),
            );
            log_rpc!(">>>> Returning {} closest peers", closest_nodes.len());

            // Send find_node answer
            let mut reply_msg = ::capnp::message::Builder::new_default();
            let mut answer = reply_msg.init_root::<veilid_capnp::operation::Builder>();
            answer.set_op_id(operation.get_op_id());
            let mut respond_to = answer.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = answer.reborrow().init_detail();
            let fna = detail.init_find_node_a();
            let mut peers_builder = fna.init_peers(
                closest_nodes
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("invalid closest nodes list length"))?,
            );
            for (i, closest_node) in closest_nodes.iter().enumerate() {
                let mut pi_builder = peers_builder.reborrow().get(i as u32);
                encode_peer_info(closest_node, &mut pi_builder)?;
            }
            reply_msg.into_reader()
        };

        self.reply(rpcreader, reply_msg, None).await
    }
}
