use crate::*;
use rpc_processor::*;

pub fn encode_node_info(
    node_info: &NodeInfo,
    builder: &mut veilid_capnp::node_info::Builder,
) -> Result<(), RPCError> {
    builder.set_network_class(encode_network_class(node_info.network_class));
    builder.set_will_route(node_info.will_route);
    builder.set_will_tunnel(node_info.will_tunnel);
    builder.set_will_signal(node_info.will_signal);
    builder.set_will_relay(node_info.will_relay);
    builder.set_will_validate_dial_info(node_info.will_validate_dial_info);

    Ok(())
}

pub fn decode_node_info(reader: &veilid_capnp::node_info::Reader) -> Result<NodeInfo, RPCError> {
    Ok(NodeInfo {
        network_class: decode_network_class(
            reader
                .reborrow()
                .get_network_class()
                .map_err(map_error_capnp_notinschema!())?,
        ),
        will_route: reader.reborrow().get_will_route(),
        will_tunnel: reader.reborrow().get_will_tunnel(),
        will_signal: reader.reborrow().get_will_signal(),
        will_relay: reader.reborrow().get_will_relay(),
        will_validate_dial_info: reader.reborrow().get_will_validate_dial_info(),
    })
}
