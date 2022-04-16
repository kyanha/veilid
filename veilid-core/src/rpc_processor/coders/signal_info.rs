use crate::*;
use rpc_processor::*;

pub fn encode_signal_info(
    signal_info: &SignalInfo,
    builder: &mut veilid_capnp::operation_signal::Builder,
) -> Result<(), RPCError> {
    match signal_info {
        SignalInfo::HolePunch { receipt, node_info } => {
            let mut hp_builder = builder.init_hole_punch();
            let rcpt_builder =
                hp_builder
                    .reborrow()
                    .init_receipt(receipt.len().try_into().map_err(map_error_protocol!(
                        "invalid receipt length in hole punch signal info"
                    ))?);
            rcpt_builder.copy_from_slice(receipt.as_slice());
            let mut ni_builder = hp_builder.init_node_info();
            encode_node_info(&node_info, &mut ni_builder)?;
        }
        SignalInfo::ReverseConnect { receipt, node_info } => {
            let mut hp_builder = builder.init_reverse_connect();
            let rcpt_builder =
                hp_builder
                    .reborrow()
                    .init_receipt(receipt.len().try_into().map_err(map_error_protocol!(
                        "invalid receipt length in reverse connect signal info"
                    ))?);
            rcpt_builder.copy_from_slice(receipt.as_slice());
            let mut ni_builder = hp_builder.init_node_info();
            encode_node_info(&node_info, &mut ni_builder)?;
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
                let receipt = r
                    .get_receipt()
                    .map_err(map_error_protocol!(
                        "invalid receipt in hole punch signal info"
                    ))?
                    .to_vec();
                let ni_reader = r.get_node_info().map_err(map_error_protocol!(
                    "invalid node info in hole punch signal info"
                ))?;
                let node_info = decode_node_info(&ni_reader, true)?;

                SignalInfo::HolePunch { receipt, node_info }
            }
            veilid_capnp::operation_signal::ReverseConnect(r) => {
                // Extract reverse connect reader
                let r = match r {
                    Ok(r) => r,
                    Err(_) => return Err(rpc_error_internal("invalid reverse connect")),
                };
                let receipt = r
                    .get_receipt()
                    .map_err(map_error_protocol!(
                        "invalid receipt in reverse connect signal info"
                    ))?
                    .to_vec();
                let ni_reader = r.get_node_info().map_err(map_error_protocol!(
                    "invalid node info in reverse connect signal info"
                ))?;
                let node_info = decode_node_info(&ni_reader, true)?;

                SignalInfo::ReverseConnect { receipt, node_info }
            }
        },
    )
}
