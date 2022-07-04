use super::*;

impl RPCProcessor {
    // Sends a unidirectional in-band return receipt
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_return_receipt<D: AsRef<[u8]>>(
        self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
        receipt: D,
    ) -> Result<(), RPCError> {
        let receipt = receipt.as_ref().to_vec();

        let return_receipt = RPCOperationReturnReceipt { receipt };
        let statement = RPCStatement::new(RPCStatementDetail::ReturnReceipt(return_receipt));

        // Send the return_receipt request
        self.statement(dest, statement, safety_route).await?;

        Ok(())
    }

    pub(crate) async fn process_return_receipt(&self, msg: RPCMessage) -> Result<(), RPCError> {
        // Get the statement
        let RPCOperationReturnReceipt { receipt } = match msg.operation.into_kind() {
            RPCOperationKind::Statement(s) => match s.into_detail() {
                RPCStatementDetail::ReturnReceipt(s) => s,
                _ => panic!("not a return receipt"),
            },
            _ => panic!("not a statement"),
        };

        // Handle it
        let network_manager = self.network_manager();
        network_manager
            .handle_in_band_receipt(receipt, msg.header.peer_noderef)
            .await
            .map_err(map_error_string!())
    }
}
