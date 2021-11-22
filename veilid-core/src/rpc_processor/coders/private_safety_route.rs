use crate::xx::*;
use crate::*;
use core::convert::TryInto;
use rpc_processor::*;

////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct RouteHopData {
    pub nonce: Nonce,
    pub blob: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct RouteHop {
    pub dial_info: NodeDialInfoSingle,
    pub next_hop: Option<RouteHopData>,
}

#[derive(Clone, Debug)]
pub struct PrivateRoute {
    pub public_key: DHTKey,
    pub hop_count: u8,
    pub hops: Option<RouteHop>,
}

#[derive(Clone, Debug)]
pub enum SafetyRouteHops {
    Data(RouteHopData),
    Private(PrivateRoute),
}

#[derive(Clone, Debug)]
pub struct SafetyRoute {
    pub public_key: DHTKey,
    pub hop_count: u8,
    pub hops: SafetyRouteHops,
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub fn encode_route_hop_data(
    route_hop_data: &RouteHopData,
    builder: &mut veilid_capnp::route_hop_data::Builder,
) -> Result<(), RPCError> {
    //
    let mut nonce_builder = builder.reborrow().init_nonce();
    encode_nonce(&route_hop_data.nonce, &mut nonce_builder);
    let blob_builder = builder.reborrow().init_blob(
        route_hop_data
            .blob
            .len()
            .try_into()
            .map_err(map_error_internal!("invalid blob length in route hop data"))?,
    );
    blob_builder.copy_from_slice(route_hop_data.blob.as_slice());
    Ok(())
}

pub fn encode_route_hop(
    route_hop: &RouteHop,
    builder: &mut veilid_capnp::route_hop::Builder,
) -> Result<(), RPCError> {
    encode_node_dial_info_single(
        &route_hop.dial_info,
        &mut builder.reborrow().init_dial_info(),
    )?;
    if let Some(rhd) = &route_hop.next_hop {
        let mut rhd_builder = builder.reborrow().init_next_hop();
        encode_route_hop_data(&rhd, &mut rhd_builder)?;
    }
    Ok(())
}

pub fn encode_private_route(
    private_route: &PrivateRoute,
    builder: &mut veilid_capnp::private_route::Builder,
) -> Result<(), RPCError> {
    encode_public_key(
        &private_route.public_key,
        &mut builder.reborrow().init_public_key(),
    )?;
    builder.set_hop_count(private_route.hop_count);
    if let Some(rh) = &private_route.hops {
        let mut rh_builder = builder.reborrow().init_first_hop();
        encode_route_hop(&rh, &mut rh_builder)?;
    };

    Ok(())
}

pub fn encode_safety_route(
    safety_route: &SafetyRoute,
    builder: &mut veilid_capnp::safety_route::Builder,
) -> Result<(), RPCError> {
    encode_public_key(
        &safety_route.public_key,
        &mut builder.reborrow().init_public_key(),
    )?;
    builder.set_hop_count(safety_route.hop_count);
    let h_builder = builder.reborrow().init_hops();
    match &safety_route.hops {
        SafetyRouteHops::Data(rhd) => {
            let mut rhd_builder = h_builder.init_data();
            encode_route_hop_data(&rhd, &mut rhd_builder)?;
        }
        SafetyRouteHops::Private(pr) => {
            let mut pr_builder = h_builder.init_private();
            encode_private_route(&pr, &mut pr_builder)?;
        }
    };

    Ok(())
}

pub fn decode_route_hop_data(
    reader: &veilid_capnp::route_hop_data::Reader,
) -> Result<RouteHopData, RPCError> {
    let nonce = decode_nonce(
        &reader
            .reborrow()
            .get_nonce()
            .map_err(map_error_internal!("invalid nonce in route hop data"))?,
    );

    let blob = reader
        .reborrow()
        .get_blob()
        .map_err(map_error_internal!("invalid blob in route hop data"))?
        .to_vec();

    Ok(RouteHopData {
        nonce: nonce,
        blob: blob,
    })
}

pub fn decode_route_hop(reader: &veilid_capnp::route_hop::Reader) -> Result<RouteHop, RPCError> {
    let dial_info = decode_node_dial_info_single(
        &reader
            .reborrow()
            .get_dial_info()
            .map_err(map_error_internal!("invalid dial info in route hop"))?,
    )?;

    let next_hop = if reader.has_next_hop() {
        let rhd_reader = reader
            .get_next_hop()
            .map_err(map_error_internal!("invalid next hop in route hop"))?;
        Some(decode_route_hop_data(&rhd_reader)?)
    } else {
        None
    };

    Ok(RouteHop {
        dial_info: dial_info,
        next_hop: next_hop,
    })
}

pub fn decode_private_route(
    reader: &veilid_capnp::private_route::Reader,
) -> Result<PrivateRoute, RPCError> {
    let public_key = decode_public_key(
        &reader
            .get_public_key()
            .map_err(map_error_internal!("invalid public key in private route"))?,
    );
    let hop_count = reader.get_hop_count();
    let hops = if reader.has_first_hop() {
        let rh_reader = reader
            .get_first_hop()
            .map_err(map_error_internal!("invalid first hop in private route"))?;
        Some(decode_route_hop(&rh_reader)?)
    } else {
        None
    };

    Ok(PrivateRoute {
        public_key: public_key,
        hop_count: hop_count,
        hops: hops,
    })
}

pub fn decode_safety_route(
    reader: &veilid_capnp::safety_route::Reader,
) -> Result<SafetyRoute, RPCError> {
    let public_key = decode_public_key(
        &reader
            .get_public_key()
            .map_err(map_error_internal!("invalid public key in safety route"))?,
    );
    let hop_count = reader.get_hop_count();
    let hops = match reader.get_hops().which() {
        Ok(veilid_capnp::safety_route::hops::Which::Data(Ok(rhd_reader))) => {
            SafetyRouteHops::Data(decode_route_hop_data(&rhd_reader)?)
        }
        Ok(veilid_capnp::safety_route::hops::Which::Private(Ok(pr_reader))) => {
            SafetyRouteHops::Private(decode_private_route(&pr_reader)?)
        }
        _ => {
            return Err(rpc_error_internal("invalid hops in safety route"));
        }
    };

    Ok(SafetyRoute {
        public_key: public_key,
        hop_count: hop_count,
        hops: hops,
    })
}
