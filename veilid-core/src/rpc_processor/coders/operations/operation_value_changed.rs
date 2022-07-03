use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationValueChanged {
    key: ValueKey,
    value: ValueData,
}

impl RPCOperationValueChanged {
    pub fn decode(
        reader: &veilid_capnp::operation_value_changed::Reader,
    ) -> Result<RPCOperationValueChanged, RPCError> {
        let k_reader = reader.get_key().map_err(map_error_capnp_error!())?;
        let key = decode_value_key(&k_reader)?;
        let v_reader = reader.get_value().map_err(map_error_capnp_error!())?;
        let value = decode_value_data(&v_reader)?;
        Ok(RPCOperationValueChanged { key, value })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_value_changed::Builder,
    ) -> Result<(), RPCError> {
        let k_builder = builder.init_key();
        encode_value_key(&self.key, &mut k_builder)?;
        let v_builder = builder.init_value();
        encode_value_data(&self.value, &mut v_builder)?;
        Ok(())
    }
}
