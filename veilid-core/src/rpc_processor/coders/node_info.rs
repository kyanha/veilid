use crate::*;
use rpc_processor::*;

pub fn encode_node_info(
    node_info: &NodeInfo,
    builder: &mut veilid_capnp::node_info::Builder,
) -> Result<(), RPCError> {
    builder.set_network_class(encode_network_class(node_info.network_class));

    let mut ps_builder = builder.reborrow().init_outbound_protocols();
    encode_protocol_set(&node_info.outbound_protocols, &mut ps_builder)?;

    let mut didl_builder = builder.reborrow().init_dial_info_detail_list(
        node_info
            .dial_info_detail_list
            .len()
            .try_into()
            .map_err(map_error_protocol!(
                "too many dial info details in node info"
            ))?,
    );

    for idx in 0..node_info.dial_info_detail_list.len() {
        let mut did_builder = didl_builder.reborrow().get(idx as u32);
        encode_dial_info_detail(&node_info.dial_info_detail_list[idx], &mut did_builder)?;
    }

    if let Some(rpi) = &node_info.relay_peer_info {
        let mut rpi_builder = builder.reborrow().init_relay_peer_info();
        encode_peer_info(rpi, &mut rpi_builder)?;
    }

    Ok(())
}

pub fn decode_node_info(
    reader: &veilid_capnp::node_info::Reader,
    allow_relay_peer_info: bool,
) -> Result<NodeInfo, RPCError> {
    let network_class = decode_network_class(
        reader
            .reborrow()
            .get_network_class()
            .map_err(map_error_capnp_notinschema!())?,
    );

    let outbound_protocols = decode_protocol_set(
        &reader
            .reborrow()
            .get_outbound_protocols()
            .map_err(map_error_capnp_error!())?,
    )?;

    let didl_reader = reader
        .reborrow()
        .get_dial_info_detail_list()
        .map_err(map_error_capnp_error!())?;
    let mut dial_info_detail_list = Vec::<DialInfo>::with_capacity(
        didl_reader
            .len()
            .try_into()
            .map_err(map_error_protocol!("too many dial info details"))?,
    );
    for di in dil_reader.iter() {
        dial_info_detail_list.push(decode_dial_info_detail(&di)?)
    }

    let relay_peer_info = if allow_relay_peer_info {
        if reader.has_relay_peer_info() {
            Some(Box::new(decode_peer_info(
                &reader
                    .reborrow()
                    .get_relay_peer_info()
                    .map_err(map_error_capnp_error!())?,
                false,
            )?))
        } else {
            None
        }
    } else {
        None
    };

    Ok(NodeInfo {
        network_class,
        outbound_protocols,
        dial_info_detail_list,
        relay_peer_info,
    })
}
