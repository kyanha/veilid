use super::*;

impl RPCProcessor {
    pub(crate) async fn process_watch_value_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_watch_value_q"))
    }
}
