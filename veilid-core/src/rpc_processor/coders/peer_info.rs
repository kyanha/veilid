use crate::xx::*;
use crate::*;
use core::convert::TryInto;
use rpc_processor::*;

pub fn encode_peer_info(
    peer_info: &PeerInfo,
    builder: &mut veilid_capnp::peer_info::Builder,
) -> Result<(), RPCError> {
    //
    let mut nid_builder = builder.reborrow().init_node_id();
    encode_public_key(&peer_info.node_id.key, &mut nid_builder)?;
    let mut dil_builder = builder.reborrow().init_dial_info_list(
        peer_info
            .dial_infos
            .len()
            .try_into()
            .map_err(map_error_internal!("too many dial infos in peer info"))?,
    );

    for idx in 0..peer_info.dial_infos.len() {
        let mut di_builder = dil_builder.reborrow().get(idx as u32);
        encode_dial_info(&peer_info.dial_infos[idx], &mut di_builder)?;
    }
    Ok(())
}

pub fn decode_peer_info(reader: &veilid_capnp::peer_info::Reader) -> Result<PeerInfo, RPCError> {
    let nid_reader = reader
        .reborrow()
        .get_node_id()
        .map_err(map_error_capnp_error!())?;
    let dil_reader = reader
        .reborrow()
        .get_dial_info_list()
        .map_err(map_error_capnp_error!())?;
    let mut dial_infos = Vec::<DialInfo>::with_capacity(
        dil_reader
            .len()
            .try_into()
            .map_err(map_error_internal!("too many dial infos"))?,
    );
    for di in dil_reader.iter() {
        dial_infos.push(decode_dial_info(&di)?)
    }
    Ok(PeerInfo {
        node_id: NodeId::new(decode_public_key(&nid_reader)),
        dial_infos: dial_infos,
    })
}
