use super::*;

impl RPCProcessor {
    pub(crate) async fn process_get_value_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_get_value_q"))
    }
}
