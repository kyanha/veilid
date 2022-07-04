use super::*;

impl RPCProcessor {
    // Sends a unidirectional signal to a node
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_signal(
        self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
        signal_info: SignalInfo,
    ) -> Result<(), RPCError> {
        //let signed_node_info = self.routing_table().get_own_signed_node_info();
        let signal = RPCOperationSignal { signal_info };
        let statement = RPCStatement::new(RPCStatementDetail::Signal(signal));

        // Send the signal request
        self.statement(dest, statement, safety_route).await?;

        Ok(())
    }

    pub(crate) async fn process_signal(&self, msg: RPCMessage) -> Result<(), RPCError> {
        // Get the statement
        let signal = match msg.operation.into_kind() {
            RPCOperationKind::Statement(s) => match s.into_detail() {
                RPCStatementDetail::Signal(s) => s,
                _ => panic!("not a node info update"),
            },
            _ => panic!("not a statement"),
        };

        // Handle it
        let network_manager = self.network_manager();
        network_manager
            .handle_signal(signal.signal_info)
            .await
            .map_err(map_error_string!())
    }
}
