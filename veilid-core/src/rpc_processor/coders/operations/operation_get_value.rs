use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationGetValueQ {
    key: ValueKey,
}

impl RPCOperationGetValueQ {
    pub fn decode(
        reader: &veilid_capnp::operation_get_value_q::Reader,
    ) -> Result<RPCOperationGetValueQ, RPCError> {
        let ni_reader = reader.get_node_id().map_err(map_error_capnp_error!())?;
        let node_id = decode_public_key(&ni_reader);
        Ok(RPCOperationGetValueQ { node_id })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_get_value_q::Builder,
    ) -> Result<(), RPCError> {
        let ni_builder = builder.init_node_id();
        encode_public_key(&self.node_id, &mut ni_builder)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum RPCOperationGetValueA {
    Data(ValueData),
    Peers(Vec<PeerInfo>),
}

impl RPCOperationGetValueA {
    pub fn decode(
        reader: &veilid_capnp::operation_get_value_a::Reader,
    ) -> Result<RPCOperationGetValueA, RPCError> {
        let peers_reader = reader.get_peers().map_err(map_error_capnp_error!())?;
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

        Ok(RPCOperationGetValueA { peers })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_get_value_a::Builder,
    ) -> Result<(), RPCError> {
        let mut peers_builder = builder.init_peers(
            self.peers
                .len()
                .try_into()
                .map_err(map_error_internal!("invalid closest nodes list length"))?,
        );
        for (i, peer) in self.peers.iter().enumerate() {
            let mut pi_builder = peers_builder.reborrow().get(i as u32);
            encode_peer_info(peer, &mut pi_builder)?;
        }
        Ok(())
    }
}
