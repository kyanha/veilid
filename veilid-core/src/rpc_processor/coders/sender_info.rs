use crate::*;
use rpc_processor::*;

pub fn encode_sender_info(
    sender_info: &SenderInfo,
    builder: &mut veilid_capnp::sender_info::Builder,
) -> Result<(), RPCError> {
    if let Some(socket_address) = &sender_info.socket_address {
        let mut sab = builder.reborrow().init_socket_address();
        encode_socket_address(socket_address, &mut sab)?;
    }
    Ok(())
}

pub fn decode_sender_info(
    reader: &veilid_capnp::sender_info::Reader,
) -> Result<SenderInfo, RPCError> {
    if !reader.has_socket_address() {
        return Err(RPCError::internal("invalid socket address type"));
    }
    let socket_address = if reader.has_socket_address() {
        Some(decode_socket_address(
            &reader
                .reborrow()
                .get_socket_address()
                .map_err(RPCError::map_internal(
                    "invalid socket address in sender_info",
                ))?,
        )?)
    } else {
        None
    };
    Ok(SenderInfo { socket_address })
}
