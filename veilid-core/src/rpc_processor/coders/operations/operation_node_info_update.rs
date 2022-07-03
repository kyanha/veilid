use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationNodeInfoUpdate {
    signed_node_info: SignedNodeInfo,
}

impl RPCOperationNodeInfoUpdate {
    pub fn decode(
        reader: &veilid_capnp::operation_node_info_update::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<RPCOperationNodeInfoUpdate, RPCError> {
        let sni_reader = reader
            .get_signed_node_info()
            .map_err(map_error_capnp_error!())?;
        let signed_node_info = decode_signed_node_info(&sni_reader, sender_node_id, true)?;

        Ok(RPCOperationNodeInfoUpdate { signed_node_info })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_node_info_update::Builder,
    ) -> Result<(), RPCError> {
        let sni_builder = builder.init_signed_node_info();
        encode_signed_node_info(&self.signed_node_info, &mut sni_builder)?;
        Ok(())
    }
}
