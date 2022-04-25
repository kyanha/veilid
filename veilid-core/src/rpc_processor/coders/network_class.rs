use crate::*;

pub fn encode_network_class(network_class: NetworkClass) -> veilid_capnp::NetworkClass {
    match network_class {
        NetworkClass::InboundCapable => veilid_capnp::NetworkClass::InboundCapable,
        NetworkClass::OutboundOnly => veilid_capnp::NetworkClass::OutboundOnly,
        NetworkClass::WebApp => veilid_capnp::NetworkClass::WebApp,
        NetworkClass::Invalid => panic!("invalid network class should not be encoded"),
    }
}

pub fn decode_network_class(network_class: veilid_capnp::NetworkClass) -> NetworkClass {
    match network_class {
        veilid_capnp::NetworkClass::InboundCapable => NetworkClass::InboundCapable,
        veilid_capnp::NetworkClass::OutboundOnly => NetworkClass::OutboundOnly,
        veilid_capnp::NetworkClass::WebApp => NetworkClass::WebApp,
    }
}
