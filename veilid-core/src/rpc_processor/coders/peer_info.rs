use crate::*;
use rpc_processor::*;

pub fn encode_peer_info(
    peer_info: &PeerInfo,
    builder: &mut veilid_capnp::peer_info::Builder,
) -> Result<(), RPCError> {
    //
    let mut nid_builder = builder.reborrow().init_node_id();
    encode_public_key(&peer_info.node_id.key, &mut nid_builder)?;
    let mut ni_builder = builder.reborrow().init_node_info();
    encode_node_info(&peer_info.node_info, &mut ni_builder)?;

    Ok(())
}

pub fn decode_peer_info(reader: &veilid_capnp::peer_info::Reader) -> Result<PeerInfo, RPCError> {
    let nid_reader = reader
        .reborrow()
        .get_node_id()
        .map_err(map_error_capnp_error!())?;
    let ni_reader = reader
        .reborrow()
        .get_node_info()
        .map_err(map_error_capnp_error!())?;
    let node_info = decode_node_info(&ni_reader)?;

    Ok(PeerInfo {
        node_id: NodeId::new(decode_public_key(&nid_reader)),
        node_info,
    })
}
