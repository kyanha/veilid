use crate::*;
use rpc_processor::*;

pub fn encode_protocol_set(
    protocol_set: &ProtocolSet,
    builder: &mut veilid_capnp::protocol_set::Builder,
) -> Result<(), RPCError> {
    builder.set_udp(protocol_set.udp);
    builder.set_tcp(protocol_set.tcp);
    builder.set_ws(protocol_set.ws);
    builder.set_wss(protocol_set.wss);

    Ok(())
}

pub fn decode_protocol_set(
    reader: &veilid_capnp::protocol_set::Reader,
) -> Result<ProtocolSet, RPCError> {
    Ok(ProtocolSet {
        udp: reader.reborrow().get_udp(),
        tcp: reader.reborrow().get_tcp(),
        ws: reader.reborrow().get_ws(),
        wss: reader.reborrow().get_wss(),
    })
}
