use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationStatusQ {
    node_status: NodeStatus,
}

impl RPCOperationStatusQ {
    pub fn decode(
        reader: &veilid_capnp::operation_status_q::Reader,
    ) -> Result<RPCOperationStatusQ, RPCError> {
        let ns_reader = reader.get_node_status().map_err(map_error_capnp_error!())?;
        let node_status = decode_node_status(&ns_reader)?;
        Ok(RPCOperationStatusQ { node_status })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_status_q::Builder,
    ) -> Result<(), RPCError> {
        let ns_builder = builder.init_node_status();
        encode_node_status(&self.node_status, &mut ns_builder)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RPCOperationStatusA {
    node_status: NodeStatus,
    sender_info: SenderInfo,
}

impl RPCOperationStatusA {
    pub fn decode(
        reader: &veilid_capnp::operation_status_a::Reader,
    ) -> Result<RPCOperationStatusA, RPCError> {
        let ns_reader = reader.get_node_status().map_err(map_error_capnp_error!())?;
        let node_status = decode_node_status(&ns_reader)?;

        let si_reader = reader
            .get_sender_info()
            .map_err(map_error_capnp_notinschema!())?;
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
        let ns_builder = builder.init_node_status();
        encode_node_status(&self.node_status, &mut ns_builder)?;
        let si_builder = builder.init_sender_info();
        encode_sender_info(&self.sender_info, &mut si_builder)?;
        Ok(())
    }
}
