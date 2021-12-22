use crate::xx::*;
use crate::*;
use core::convert::TryInto;
use rpc_processor::*;

pub fn decode_dial_info(reader: &veilid_capnp::dial_info::Reader) -> Result<DialInfo, RPCError> {
    match reader.reborrow().which() {
        Ok(veilid_capnp::dial_info::Which::Udp(Ok(udp))) => {
            let socket_address_reader = udp
                .get_socket_address()
                .map_err(map_error_protocol!("missing UDP socketAddress"))?;
            let socket_address = decode_socket_address(&socket_address_reader)?;
            Ok(DialInfo::udp(socket_address))
        }
        Ok(veilid_capnp::dial_info::Which::Tcp(Ok(tcp))) => {
            let socket_address_reader = tcp
                .get_socket_address()
                .map_err(map_error_protocol!("missing TCP socketAddress"))?;
            let socket_address = decode_socket_address(&socket_address_reader)?;
            Ok(DialInfo::tcp(socket_address))
        }
        Ok(veilid_capnp::dial_info::Which::Ws(Ok(ws))) => {
            let socket_address_reader = ws
                .get_socket_address()
                .map_err(map_error_protocol!("missing WS socketAddress"))?;
            let socket_address = decode_socket_address(&socket_address_reader)?;
            let request = ws
                .get_request()
                .map_err(map_error_protocol!("missing WS request"))?;
            DialInfo::try_ws(socket_address, request.to_owned())
                .map_err(map_error_protocol!("invalid WS dial info"))
        }
        Ok(veilid_capnp::dial_info::Which::Wss(Ok(wss))) => {
            let socket_address_reader = wss
                .get_socket_address()
                .map_err(map_error_protocol!("missing WSS socketAddress"))?;
            let socket_address = decode_socket_address(&socket_address_reader)?;
            let request = wss
                .get_request()
                .map_err(map_error_protocol!("missing WSS request"))?;
            DialInfo::try_wss(socket_address, request.to_owned())
                .map_err(map_error_protocol!("invalid WSS dial info"))
        }
        _ => Err(rpc_error_internal("invalid dial info type")),
    }
}

pub fn encode_dial_info(
    dial_info: &DialInfo,
    builder: &mut veilid_capnp::dial_info::Builder,
) -> Result<(), RPCError> {
    match dial_info {
        DialInfo::UDP(udp) => {
            let mut di_udp_builder = builder.reborrow().init_udp();
            encode_socket_address(
                &udp.socket_address,
                &mut di_udp_builder.reborrow().init_socket_address(),
            )?;
        }
        DialInfo::TCP(tcp) => {
            let mut di_tcp_builder = builder.reborrow().init_tcp();
            encode_socket_address(
                &tcp.socket_address,
                &mut di_tcp_builder.reborrow().init_socket_address(),
            )?;
        }
        DialInfo::WS(ws) => {
            let mut di_ws_builder = builder.reborrow().init_ws();
            encode_socket_address(
                &ws.socket_address,
                &mut di_ws_builder.reborrow().init_socket_address(),
            )?;
            let mut requestb = di_ws_builder.init_request(
                ws.request
                    .len()
                    .try_into()
                    .map_err(map_error_protocol!("request too long"))?,
            );
            requestb.push_str(ws.request.as_str());
        }
        DialInfo::WSS(wss) => {
            let mut di_wss_builder = builder.reborrow().init_wss();
            encode_socket_address(
                &wss.socket_address,
                &mut di_wss_builder.reborrow().init_socket_address(),
            )?;
            let mut requestb = di_wss_builder.init_request(
                wss.request
                    .len()
                    .try_into()
                    .map_err(map_error_protocol!("request too long"))?,
            );
            requestb.push_str(wss.request.as_str());
        }
    };
    Ok(())
}
