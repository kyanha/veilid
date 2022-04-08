use crate::*;
use rpc_processor::*;

pub fn encode_node_status(
    node_status: &NodeStatus,
    builder: &mut veilid_capnp::node_status::Builder,
) -> Result<(), RPCError> {
    builder.set_will_route(node_status.will_route);
    builder.set_will_tunnel(node_status.will_tunnel);
    builder.set_will_signal(node_status.will_signal);
    builder.set_will_relay(node_status.will_relay);
    builder.set_will_validate_dial_info(node_status.will_validate_dial_info);

    Ok(())
}

pub fn decode_node_status(
    reader: &veilid_capnp::node_status::Reader,
) -> Result<NodeStatus, RPCError> {
    Ok(NodeStatus {
        will_route: reader.reborrow().get_will_route(),
        will_tunnel: reader.reborrow().get_will_tunnel(),
        will_signal: reader.reborrow().get_will_signal(),
        will_relay: reader.reborrow().get_will_relay(),
        will_validate_dial_info: reader.reborrow().get_will_validate_dial_info(),
    })
}
