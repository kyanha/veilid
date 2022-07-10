use super::*;

impl RPCProcessor {
    pub(crate) async fn process_set_value_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_set_value_q"))
    }
}
