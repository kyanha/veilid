use super::*;

impl RPCProcessor {
    pub(crate) async fn process_start_tunnel_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_start_tunnel_q"))
    }
}
