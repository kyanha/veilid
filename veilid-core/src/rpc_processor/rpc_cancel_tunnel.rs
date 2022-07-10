use super::*;

impl RPCProcessor {
    pub(crate) async fn process_cancel_tunnel_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        Err(RPCError::unimplemented("process_cancel_tunnel_q"))
    }
}
