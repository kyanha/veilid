use super::*;

impl RPCProcessor {
    // xxx do not process latency for routed messages

    pub(crate) async fn process_route(&self, _rpcreader: RPCMessage) -> Result<(), RPCError> {
        // xxx do not process latency for routed messages
        Err(rpc_error_unimplemented("process_route"))
    }
}
