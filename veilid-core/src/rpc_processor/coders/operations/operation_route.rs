use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
struct RoutedOperation {
    pub signatures: Vec<DHTSignature>,
    pub nonce: Nonce,
    pub data: Vec<u8>,
}

impl RoutedOperation {
    pub fn decode(
        reader: &veilid_capnp::routed_operation::Reader,
    ) -> Result<RoutedOperation, RPCError> {
        let sigs_reader = reader.get_signatures().map_err(map_error_capnp_error!())?;
        let mut signatures = Vec::<DHTSignature>::with_capacity(
            sigs_reader
                .len()
                .try_into()
                .map_err(map_error_internal!("too many signatures"))?,
        );
        for s in sigs_reader.iter() {
            let sig = decode_signature(&s);
            signatures.push(sig);
        }

        let n_reader = reader.get_nonce().map_err(map_error_capnp_error!())?;
        let nonce = decode_nonce(&n_reader);
        let data = reader
            .get_data()
            .map_err(map_error_capnp_error!())?
            .to_vec();

        Ok(RoutedOperation {
            signatures,
            nonce,
            data,
        })
    }

    pub fn encode(
        &self,
        builder: &mut veilid_capnp::routed_operation::Builder,
    ) -> Result<(), RPCError> {
        let mut sigs_builder = builder.init_signatures(
            self.signatures
                .len()
                .try_into()
                .map_err(map_error_internal!("invalid signatures list length"))?,
        );
        for (i, sig) in self.signatures.iter().enumerate() {
            let mut sig_builder = sigs_builder.reborrow().get(i as u32);
            encode_signature(sig, &mut sig_builder);
        }
        let n_builder = builder.init_nonce();
        encode_nonce(&self.nonce, &mut n_builder);
        builder.set_data(&self.data);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RPCOperationRoute {
    pub safety_route: SafetyRoute,
    pub operation: RoutedOperation,
}

impl RPCOperationRoute {
    pub fn decode(
        reader: &veilid_capnp::operation_route::Reader,
    ) -> Result<RPCOperationRoute, RPCError> {
        let sr_reader = reader
            .get_safety_route()
            .map_err(map_error_capnp_error!())?;
        let safety_route = decode_safety_route(&sr_reader)?;

        let o_reader = reader.get_operation().map_err(map_error_capnp_error!())?;
        let operation = RoutedOperation::decode(&o_reader)?;

        Ok(RPCOperationRoute {
            safety_route,
            operation,
        })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_route::Builder,
    ) -> Result<(), RPCError> {
        let sr_builder = builder.init_safety_route();
        encode_safety_route(&self.safety_route, &mut sr_builder)?;
        let o_builder = builder.init_operation();
        self.operation.encode(&mut o_builder)?;
        Ok(())
    }
}
