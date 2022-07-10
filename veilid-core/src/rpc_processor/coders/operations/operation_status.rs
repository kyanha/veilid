use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationStatusQ {
    pub node_status: NodeStatus,
}

impl RPCOperationStatusQ {
    pub fn decode(
        reader: &veilid_capnp::operation_status_q::Reader,
    ) -> Result<RPCOperationStatusQ, RPCError> {
        let ns_reader = reader.get_node_status().map_err(RPCError::protocol)?;
        let node_status = decode_node_status(&ns_reader)?;
        Ok(RPCOperationStatusQ { node_status })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_status_q::Builder,
    ) -> Result<(), RPCError> {
        let mut ns_builder = builder.reborrow().init_node_status();
        encode_node_status(&self.node_status, &mut ns_builder)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RPCOperationStatusA {
    pub node_status: NodeStatus,
    pub sender_info: SenderInfo,
}

impl RPCOperationStatusA {
    pub fn decode(
        reader: &veilid_capnp::operation_status_a::Reader,
    ) -> Result<RPCOperationStatusA, RPCError> {
        let ns_reader = reader.get_node_status().map_err(RPCError::protocol)?;
        let node_status = decode_node_status(&ns_reader)?;

        let si_reader = reader.get_sender_info().map_err(RPCError::protocol)?;
        let sender_info = decode_sender_info(&si_reader)?;

        Ok(RPCOperationStatusA {
            node_status,
            sender_info,
        })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_status_a::Builder,
    ) -> Result<(), RPCError> {
        let mut ns_builder = builder.reborrow().init_node_status();
        encode_node_status(&self.node_status, &mut ns_builder)?;
        let mut si_builder = builder.reborrow().init_sender_info();
        encode_sender_info(&self.sender_info, &mut si_builder)?;
        Ok(())
    }
}
