use crate::*;
use rpc_processor::*;

pub fn encode_node_info(
    node_info: &NodeInfo,
    builder: &mut veilid_capnp::node_info::Builder,
) -> Result<(), RPCError> {
    builder.set_can_route(node_info.can_route);
    builder.set_will_route(node_info.will_route);

    builder.set_can_tunnel(node_info.can_tunnel);
    builder.set_will_tunnel(node_info.will_tunnel);

    builder.set_can_signal_lease(node_info.can_signal_lease);
    builder.set_will_signal_lease(node_info.will_signal_lease);

    builder.set_can_relay_lease(node_info.can_relay_lease);
    builder.set_will_relay_lease(node_info.will_relay_lease);

    builder.set_can_validate_dial_info(node_info.can_validate_dial_info);
    builder.set_will_validate_dial_info(node_info.will_validate_dial_info);

    Ok(())
}

pub fn decode_node_info(reader: &veilid_capnp::node_info::Reader) -> Result<NodeInfo, RPCError> {
    Ok(NodeInfo {
        can_route: reader.reborrow().get_can_route(),
        will_route: reader.reborrow().get_will_route(),
        can_tunnel: reader.reborrow().get_can_tunnel(),
        will_tunnel: reader.reborrow().get_will_tunnel(),
        can_signal_lease: reader.reborrow().get_can_signal_lease(),
        will_signal_lease: reader.reborrow().get_will_signal_lease(),
        can_relay_lease: reader.reborrow().get_can_relay_lease(),
        will_relay_lease: reader.reborrow().get_will_relay_lease(),
        can_validate_dial_info: reader.reborrow().get_can_validate_dial_info(),
        will_validate_dial_info: reader.reborrow().get_will_validate_dial_info(),
    })
}
