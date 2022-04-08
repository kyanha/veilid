use crate::*;
use rpc_processor::*;

pub fn encode_node_info(
    node_info: &NodeInfo,
    builder: &mut veilid_capnp::node_info::Builder,
) -> Result<(), RPCError> {
    builder.set_network_class(encode_network_class(node_info.network_class));

    let mut dil_builder = builder.reborrow().init_dial_info_list(
        node_info
            .dial_infos
            .len()
            .try_into()
            .map_err(map_error_protocol!("too many dial infos in node info"))?,
    );

    for idx in 0..node_info.dial_infos.len() {
        let mut di_builder = dil_builder.reborrow().get(idx as u32);
        encode_dial_info(&node_info.dial_infos[idx], &mut di_builder)?;
    }

    let mut rdil_builder = builder.reborrow().init_relay_dial_info_list(
        node_info
            .relay_dial_infos
            .len()
            .try_into()
            .map_err(map_error_protocol!(
                "too many relay dial infos in node info"
            ))?,
    );

    for idx in 0..node_info.relay_dial_infos.len() {
        let mut rdi_builder = rdil_builder.reborrow().get(idx as u32);
        encode_dial_info(&node_info.relay_dial_infos[idx], &mut rdi_builder)?;
    }

    Ok(())
}

pub fn decode_node_info(reader: &veilid_capnp::node_info::Reader) -> Result<NodeInfo, RPCError> {
    let network_class = decode_network_class(
        reader
            .reborrow()
            .get_network_class()
            .map_err(map_error_capnp_notinschema!())?,
    );

    let dil_reader = reader
        .reborrow()
        .get_dial_info_list()
        .map_err(map_error_capnp_error!())?;
    let mut dial_infos = Vec::<DialInfo>::with_capacity(
        dil_reader
            .len()
            .try_into()
            .map_err(map_error_protocol!("too many dial infos"))?,
    );
    for di in dil_reader.iter() {
        dial_infos.push(decode_dial_info(&di)?)
    }

    let rdil_reader = reader
        .reborrow()
        .get_relay_dial_info_list()
        .map_err(map_error_capnp_error!())?;
    let mut relay_dial_infos = Vec::<DialInfo>::with_capacity(
        rdil_reader
            .len()
            .try_into()
            .map_err(map_error_protocol!("too many relay dial infos"))?,
    );
    for di in rdil_reader.iter() {
        relay_dial_infos.push(decode_dial_info(&di)?)
    }

    Ok(NodeInfo {
        network_class,
        dial_infos,
        relay_dial_infos,
    })
}
