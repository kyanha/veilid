use super::*;

impl RPCProcessor {
    pub(crate) async fn process_supply_block_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_supply_block_q"))
    }
}
