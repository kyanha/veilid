use crate::*;

pub fn encode_network_class(network_class: NetworkClass) -> veilid_capnp::NetworkClass {
    match network_class {
        NetworkClass::Server => veilid_capnp::NetworkClass::Server,
        NetworkClass::Mapped => veilid_capnp::NetworkClass::Mapped,
        NetworkClass::FullConeNAT => veilid_capnp::NetworkClass::FullConeNAT,
        NetworkClass::AddressRestrictedNAT => veilid_capnp::NetworkClass::AddressRestrictedNAT,
        NetworkClass::PortRestrictedNAT => veilid_capnp::NetworkClass::PortRestrictedNAT,
        NetworkClass::OutboundOnly => veilid_capnp::NetworkClass::OutboundOnly,
        NetworkClass::WebApp => veilid_capnp::NetworkClass::WebApp,
        NetworkClass::Invalid => veilid_capnp::NetworkClass::Invalid,
    }
}

pub fn decode_network_class(network_class: veilid_capnp::NetworkClass) -> NetworkClass {
    match network_class {
        veilid_capnp::NetworkClass::Server => NetworkClass::Server,
        veilid_capnp::NetworkClass::Mapped => NetworkClass::Mapped,
        veilid_capnp::NetworkClass::FullConeNAT => NetworkClass::FullConeNAT,
        veilid_capnp::NetworkClass::AddressRestrictedNAT => NetworkClass::AddressRestrictedNAT,
        veilid_capnp::NetworkClass::PortRestrictedNAT => NetworkClass::PortRestrictedNAT,
        veilid_capnp::NetworkClass::OutboundOnly => NetworkClass::OutboundOnly,
        veilid_capnp::NetworkClass::WebApp => NetworkClass::WebApp,
        veilid_capnp::NetworkClass::Invalid => NetworkClass::Invalid,
    }
}
