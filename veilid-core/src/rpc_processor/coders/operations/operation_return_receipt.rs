use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationReturnReceipt {
    receipt: Vec<u8>,
}

impl RPCOperationReturnReceipt {
    pub fn decode(
        reader: &veilid_capnp::operation_return_receipt::Reader,
    ) -> Result<RPCOperationReturnReceipt, RPCError> {
        let rcpt_reader = reader.get_receipt().map_err(map_error_capnp_error!())?;
        let receipt = rcpt_reader.to_vec();

        Ok(RPCOperationReturnReceipt { receipt })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_return_receipt::Builder,
    ) -> Result<(), RPCError> {
        builder.set_receipt(&self.receipt);
        Ok(())
    }
}
