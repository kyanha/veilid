use super::*;

impl RPCProcessor {
    // Sends a unidirectional in-band return receipt
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_return_receipt<D: AsRef<[u8]>>(
        &self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
        receipt: D,
    ) -> Result<(), RPCError> {
        let receipt = receipt.as_ref();

        let rr_msg = {
            let mut rr_msg = ::capnp::message::Builder::new_default();
            let mut question = rr_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let rr_builder = detail.init_return_receipt();
            let r_builder = rr_builder.init_receipt(receipt.len().try_into().map_err(
                map_error_protocol!("invalid receipt length in return receipt"),
            )?);
            r_builder.copy_from_slice(receipt);

            rr_msg.into_reader()
        };

        // Send the return receipt request
        self.request(dest, rr_msg, safety_route).await?;

        Ok(())
    }

    pub(crate) async fn process_return_receipt(
        &self,
        rpcreader: RPCMessage,
    ) -> Result<(), RPCError> {
        let receipt = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(rpc_error_invalid_format(
                    "return receipt should not want answer",
                ));
            }

            // get returnReceipt reader
            let rr_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::ReturnReceipt(Ok(x))) => x,
                _ => panic!("invalid operation type in process_return_receipt"),
            };

            // Get receipt
            rr_reader
                .get_receipt()
                .map_err(map_error_internal!(
                    "no valid receipt in process_return_receipt"
                ))?
                .to_vec()
        };

        // Handle it
        let network_manager = self.network_manager();
        network_manager
            .handle_in_band_receipt(receipt, rpcreader.header.peer_noderef)
            .await
            .map_err(map_error_string!())
    }
}
