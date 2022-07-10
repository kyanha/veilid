use super::*;

impl RPCProcessor {
    pub(crate) async fn process_supply_block_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_supply_block_q"))
    }
}
