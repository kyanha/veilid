use super::*;

impl RPCProcessor {
    pub(crate) async fn process_value_changed(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_value_changed"))
    }
}
