use crate::xx::*;
use crate::*;
use core::convert::TryInto;
use rpc_processor::*;

pub fn decode_dial_info(reader: &veilid_capnp::dial_info::Reader) -> Result<DialInfo, RPCError> {
    match reader.reborrow().which() {
        Ok(veilid_capnp::dial_info::Which::Udp(Ok(udp))) => {
            let address_reader = udp
                .get_address()
                .map_err(map_error_internal!("missing udp address"))?;
            let address = decode_address(&address_reader)?;
            let port = udp.get_port();
            Ok(DialInfo::udp(address, port))
        }
        Ok(veilid_capnp::dial_info::Which::Tcp(Ok(tcp))) => {
            let address_reader = tcp
                .get_address()
                .map_err(map_error_internal!("missing tcp address"))?;
            let address = decode_address(&address_reader)?;
            let port = tcp.get_port();
            Ok(DialInfo::tcp(address, port))
        }
        Ok(veilid_capnp::dial_info::Which::Ws(Ok(ws))) => {
            let host = ws
                .get_host()
                .map_err(map_error_internal!("missing ws host"))?;
            let port = ws.get_port();
            let path = ws
                .get_path()
                .map_err(map_error_internal!("missing ws path"))?;
            Ok(DialInfo::ws(host.to_owned(), port, path.to_owned()))
        }
        Ok(veilid_capnp::dial_info::Which::Wss(Ok(wss))) => {
            let host = wss
                .get_host()
                .map_err(map_error_internal!("missing wss host"))?;
            let port = wss.get_port();
            let path = wss
                .get_path()
                .map_err(map_error_internal!("missing wss path"))?;
            Ok(DialInfo::wss(host.to_owned(), port, path.to_owned()))
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
            encode_address(&udp.address, &mut di_udp_builder.reborrow().init_address())?;
            di_udp_builder.set_port(udp.port);
        }
        DialInfo::TCP(tcp) => {
            let mut di_tcp_builder = builder.reborrow().init_tcp();
            encode_address(&tcp.address, &mut di_tcp_builder.reborrow().init_address())?;
            di_tcp_builder.set_port(tcp.port);
        }
        DialInfo::WS(ws) => {
            let mut di_ws_builder = builder.reborrow().init_ws();
            let mut hostb = di_ws_builder.reborrow().init_host(
                ws.host
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("host too long"))?,
            );
            hostb.push_str(ws.host.as_str());
            di_ws_builder.set_port(ws.port);
            let mut pathb = di_ws_builder.init_path(
                ws.path
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("path too long"))?,
            );
            pathb.push_str(ws.path.as_str());
        }
        DialInfo::WSS(wss) => {
            let mut di_wss_builder = builder.reborrow().init_wss();
            let mut hostb = di_wss_builder.reborrow().init_host(
                wss.host
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("host too long"))?,
            );
            hostb.push_str(wss.host.as_str());
            di_wss_builder.set_port(wss.port);
            let mut pathb = di_wss_builder.init_path(
                wss.path
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("path too long"))?,
            );
            pathb.push_str(wss.path.as_str());
        }
    };
    Ok(())
}
