use crate::*;
use rpc_processor::*;

pub fn encode_signed_node_info(
    signed_node_info: &SignedNodeInfo,
    builder: &mut veilid_capnp::signed_node_info::Builder,
) -> Result<(), RPCError> {
    //
    let mut ni_builder = builder.reborrow().init_node_info();
    encode_node_info(&signed_node_info.node_info, &mut ni_builder)?;

    let mut sig_builder = builder.reborrow().init_signature();
    encode_signature(&signed_node_info.signature, &mut sig_builder);

    Ok(())
}

pub fn decode_signed_node_info(
    reader: &veilid_capnp::signed_node_info::Reader,
    node_id: &DHTKey,
    allow_relay_peer_info: bool,
) -> Result<SignedNodeInfo, RPCError> {
    let ni_reader = reader
        .reborrow()
        .get_node_info()
        .map_err(map_error_capnp_error!())?;
    let node_info = decode_node_info(&ni_reader, allow_relay_peer_info)?;

    let sig_reader = reader
        .reborrow()
        .get_signature()
        .map_err(map_error_capnp_error!())?;
    let signature = decode_signature(&sig_reader);

    SignedNodeInfo::new(node_info, NodeId::new(*node_id), signature).map_err(map_error_string!())
}
