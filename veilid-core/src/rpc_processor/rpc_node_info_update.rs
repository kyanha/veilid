use super::*;

impl RPCProcessor {
    // Sends a our node info to another node
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_node_info_update(
        &self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
    ) -> Result<(), RPCError> {
        let sni_msg = {
            let mut sni_msg = ::capnp::message::Builder::new_default();
            let mut question = sni_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let niu_builder = detail.init_node_info_update();
            let mut sni_builder = niu_builder.init_signed_node_info();
            let sni = self.routing_table().get_own_signed_node_info();
            encode_signed_node_info(&sni, &mut sni_builder)?;

            sni_msg.into_reader()
        };

        // Send the node_info_update request
        self.request(dest, sni_msg, safety_route).await?;

        Ok(())
    }

    pub(crate) async fn process_node_info_update(
        &self,
        rpcreader: RPCMessage,
    ) -> Result<(), RPCError> {
        //
        let sender_node_id = rpcreader.header.envelope.get_sender_id();
        let signed_node_info = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(rpc_error_invalid_format(
                    "node_info_update should not want answer",
                ));
            }

            // get nodeInfoUpdate reader
            let niumsg_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::NodeInfoUpdate(Ok(x))) => x,
                _ => panic!("invalid operation type in process_node_info_update"),
            };

            // Parse out fields
            let sni_reader = niumsg_reader
                .get_signed_node_info()
                .map_err(map_error_internal!("no valid signed node info"))?;
            decode_signed_node_info(&sni_reader, &sender_node_id, true)?
        };

        // Update our routing table with signed node info
        if !self.filter_peer_scope(&signed_node_info.node_info) {
            return Err(rpc_error_invalid_format(
                "node_info_update has invalid peer scope",
            ));
        }
        let _ = self
            .routing_table()
            .register_node_with_signed_node_info(sender_node_id, signed_node_info)
            .map_err(RPCError::Internal)?;

        Ok(())
    }
}
