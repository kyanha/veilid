use super::*;

impl RPCProcessor {
    // Sends a unidirectional signal to a node
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_signal(
        &self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
        signal_info: SignalInfo,
    ) -> Result<(), RPCError> {
        let sig_msg = {
            let mut sig_msg = ::capnp::message::Builder::new_default();
            let mut question = sig_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let mut sig_builder = detail.init_signal();
            encode_signal_info(&signal_info, &mut sig_builder)?;

            sig_msg.into_reader()
        };

        // Send the signal request
        self.request(dest, sig_msg, safety_route).await?;

        Ok(())
    }

    pub(crate) async fn process_signal(&self, rpcreader: RPCMessage) -> Result<(), RPCError> {
        let signal_info = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(rpc_error_invalid_format("signal should not want answer"));
            }

            // get signal reader
            let sig_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::Signal(Ok(x))) => x,
                _ => panic!("invalid operation type in process_signal"),
            };

            // Get signal info
            decode_signal_info(&sig_reader)?
        };

        // Handle it
        let network_manager = self.network_manager();
        network_manager
            .handle_signal(signal_info)
            .await
            .map_err(map_error_string!())
    }
}
