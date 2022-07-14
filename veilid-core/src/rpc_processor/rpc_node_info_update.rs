use super::*;

impl RPCProcessor {
    // Sends a our node info to another node
    // Can be sent via all methods including relays and routes
    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn rpc_call_node_info_update(
        self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
    ) -> Result<(), RPCError> {
        let signed_node_info = self.routing_table().get_own_signed_node_info();
        let node_info_update = RPCOperationNodeInfoUpdate { signed_node_info };
        let statement = RPCStatement::new(RPCStatementDetail::NodeInfoUpdate(node_info_update));

        // Send the node_info_update request
        self.statement(dest, statement, safety_route).await?;

        Ok(())
    }

    pub(crate) async fn process_node_info_update(&self, msg: RPCMessage) -> Result<(), RPCError> {
        let sender_node_id = msg.header.envelope.get_sender_id();

        // Get the statement
        let node_info_update = match msg.operation.into_kind() {
            RPCOperationKind::Statement(s) => match s.into_detail() {
                RPCStatementDetail::NodeInfoUpdate(s) => s,
                _ => panic!("not a node info update"),
            },
            _ => panic!("not a statement"),
        };

        // Update our routing table with signed node info
        if !self.filter_peer_scope(&node_info_update.signed_node_info.node_info) {
            return Err(RPCError::invalid_format(
                "node_info_update has invalid peer scope",
            ));
        }
        let _ = self
            .routing_table()
            .register_node_with_signed_node_info(sender_node_id, node_info_update.signed_node_info)
            .map_err(map_to_string)
            .map_err(RPCError::Internal)?;

        Ok(())
    }
}
