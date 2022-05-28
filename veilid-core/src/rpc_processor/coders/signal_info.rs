use crate::*;
use rpc_processor::*;

pub fn encode_signal_info(
    signal_info: &SignalInfo,
    builder: &mut veilid_capnp::operation_signal::Builder,
) -> Result<(), RPCError> {
    match signal_info {
        SignalInfo::HolePunch {
            receipt_nonce,
            peer_info,
        } => {
            let mut hp_builder = builder.reborrow().init_hole_punch();
            let mut rn_builder = hp_builder.reborrow().init_receipt_nonce();
            encode_nonce(receipt_nonce, &mut rn_builder);
            let mut pi_builder = hp_builder.init_peer_info();
            encode_peer_info(peer_info, &mut pi_builder)?;
        }
        SignalInfo::ReverseConnect {
            receipt_nonce,
            peer_info,
        } => {
            let mut rc_builder = builder.reborrow().init_reverse_connect();
            let mut rn_builder = rc_builder.reborrow().init_receipt_nonce();
            encode_nonce(receipt_nonce, &mut rn_builder);
            let mut pi_builder = rc_builder.init_peer_info();
            encode_peer_info(peer_info, &mut pi_builder)?;
        }
    }

    Ok(())
}

pub fn decode_signal_info(
    reader: &veilid_capnp::operation_signal::Reader,
) -> Result<SignalInfo, RPCError> {
    Ok(
        match reader
            .which()
            .map_err(map_error_internal!("invalid signal operation"))?
        {
            veilid_capnp::operation_signal::HolePunch(r) => {
                // Extract hole punch reader
                let r = match r {
                    Ok(r) => r,
                    Err(_) => return Err(rpc_error_internal("invalid hole punch")),
                };
                let receipt_nonce =
                    decode_nonce(&r.get_receipt_nonce().map_err(map_error_capnp_error!())?);
                let pi_reader = r.get_peer_info().map_err(map_error_protocol!(
                    "invalid peer info in hole punch signal info"
                ))?;
                let peer_info = decode_peer_info(&pi_reader, true)?;

                SignalInfo::HolePunch {
                    receipt_nonce,
                    peer_info,
                }
            }
            veilid_capnp::operation_signal::ReverseConnect(r) => {
                // Extract reverse connect reader
                let r = match r {
                    Ok(r) => r,
                    Err(_) => return Err(rpc_error_internal("invalid reverse connect")),
                };
                let receipt_nonce =
                    decode_nonce(&r.get_receipt_nonce().map_err(map_error_capnp_error!())?);
                let pi_reader = r.get_peer_info().map_err(map_error_protocol!(
                    "invalid peer info in reverse connect signal info"
                ))?;
                let peer_info = decode_peer_info(&pi_reader, true)?;

                SignalInfo::ReverseConnect {
                    receipt_nonce,
                    peer_info,
                }
            }
        },
    )
}
