use super::*;

impl RPCProcessor {
    pub(crate) async fn process_complete_tunnel_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_complete_tunnel_q"))
    }
}
