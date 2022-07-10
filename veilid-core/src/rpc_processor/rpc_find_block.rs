use super::*;

impl RPCProcessor {
    pub(crate) async fn process_find_block_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_find_block_q"))
    }
}
