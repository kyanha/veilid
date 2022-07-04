use super::*;

impl RPCProcessor {
    pub(crate) async fn process_value_changed(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_value_changed"))
    }
}
