use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationSetValueQ {
    key: ValueKey,
    value: ValueData,
}

impl RPCOperationSetValueQ {
    pub fn decode(
        reader: &veilid_capnp::operation_set_value_q::Reader,
    ) -> Result<RPCOperationSetValueQ, RPCError> {
        let k_reader = reader.get_key().map_err(map_error_capnp_error!())?;
        let key = decode_value_key(&k_reader)?;
        let v_reader = reader.get_value().map_err(map_error_capnp_error!())?;
        let value = decode_value_data(&v_reader)?;
        Ok(RPCOperationSetValueQ { key, value })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_set_value_q::Builder,
    ) -> Result<(), RPCError> {
        let k_builder = builder.init_key();
        encode_value_key(&self.key, &mut k_builder)?;
        let v_builder = builder.init_value();
        encode_value_data(&self.value, &mut v_builder)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum RPCOperationSetValueA {
    Data(ValueData),
    Peers(Vec<PeerInfo>),
}

impl RPCOperationSetValueA {
    pub fn decode(
        reader: &veilid_capnp::operation_set_value_a::Reader,
    ) -> Result<RPCOperationSetValueA, RPCError> {
        match reader.which().map_err(map_error_capnp_notinschema!())? {
            veilid_capnp::operation_set_value_a::Which::Data(r) => {
                let data = decode_value_data(&r.map_err(map_error_capnp_error!())?)?;
                Ok(RPCOperationSetValueA::Data(data))
            }
            veilid_capnp::operation_set_value_a::Which::Peers(r) => {
                let peers_reader = r.map_err(map_error_capnp_error!())?;
                let mut peers = Vec::<PeerInfo>::with_capacity(
                    peers_reader
                        .len()
                        .try_into()
                        .map_err(map_error_internal!("too many peers"))?,
                );
                for p in peers_reader.iter() {
                    let peer_info = decode_peer_info(&p, true)?;
                    peers.push(peer_info);
                }

                Ok(RPCOperationSetValueA::Peers(peers))
            }
        }
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_set_value_a::Builder,
    ) -> Result<(), RPCError> {
        match self {
            RPCOperationSetValueA::Data(data) => {
                let d_builder = builder.init_data();
                encode_value_data(&data, &mut d_builder)?;
            }
            RPCOperationSetValueA::Peers(peers) => {
                let mut peers_builder = builder.init_peers(
                    peers
                        .len()
                        .try_into()
                        .map_err(map_error_internal!("invalid peers list length"))?,
                );
                for (i, peer) in peers.iter().enumerate() {
                    let mut pi_builder = peers_builder.reborrow().get(i as u32);
                    encode_peer_info(peer, &mut pi_builder)?;
                }
            }
        }

        Ok(())
    }
}
