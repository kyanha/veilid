use super::*;

impl RPCProcessor {
    // xxx do not process latency for routed messages

    pub(crate) async fn process_route(&self, _rpcreader: RPCMessage) -> Result<(), RPCError> {
        // xxx do not process latency for routed messages
        Err(RPCError::unimplemented("process_route"))
    }
}
