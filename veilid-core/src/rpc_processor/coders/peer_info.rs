use crate::*;
use rpc_processor::*;

pub fn encode_peer_info(
    peer_info: &PeerInfo,
    builder: &mut veilid_capnp::peer_info::Builder,
) -> Result<(), RPCError> {
    //
    let mut nid_builder = builder.reborrow().init_node_id();
    encode_public_key(&peer_info.node_id.key, &mut nid_builder)?;
    let mut sni_builder = builder.reborrow().init_signed_node_info();
    encode_signed_node_info(&peer_info.signed_node_info, &mut sni_builder)?;

    Ok(())
}

pub fn decode_peer_info(
    reader: &veilid_capnp::peer_info::Reader,
    allow_relay_peer_info: bool,
) -> Result<PeerInfo, RPCError> {
    let nid_reader = reader
        .reborrow()
        .get_node_id()
        .map_err(map_error_capnp_error!())?;
    let sni_reader = reader
        .reborrow()
        .get_signed_node_info()
        .map_err(map_error_capnp_error!())?;
    let node_id = NodeId::new(decode_public_key(&nid_reader));
    let signed_node_info =
        decode_signed_node_info(&sni_reader, &node_id.key, allow_relay_peer_info)?;

    Ok(PeerInfo {
        node_id,
        signed_node_info,
    })
}
