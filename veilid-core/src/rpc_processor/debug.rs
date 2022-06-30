use super::*;

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub enum RPCError {
    Timeout,
    InvalidFormat(String),
    Unreachable(DHTKey),
    Unimplemented(String),
    Protocol(String),
    Internal(String),
}

pub fn rpc_error_internal<T: AsRef<str>>(x: T) -> RPCError {
    error!("RPCError Internal: {}", x.as_ref());
    RPCError::Internal(x.as_ref().to_owned())
}
pub fn rpc_error_invalid_format<T: AsRef<str>>(x: T) -> RPCError {
    error!("RPCError Invalid Format: {}", x.as_ref());
    RPCError::InvalidFormat(x.as_ref().to_owned())
}
pub fn rpc_error_protocol<T: AsRef<str>>(x: T) -> RPCError {
    error!("RPCError Protocol: {}", x.as_ref());
    RPCError::Protocol(x.as_ref().to_owned())
}
pub fn rpc_error_capnp_error(e: capnp::Error) -> RPCError {
    error!("RPCError Protocol: capnp error: {}", &e.description);
    panic!("wtf");
    RPCError::Protocol(e.description)
}
pub fn rpc_error_capnp_notinschema(e: capnp::NotInSchema) -> RPCError {
    error!("RPCError Protocol: not in schema: {}", &e.0);
    RPCError::Protocol(format!("not in schema: {}", &e.0))
}
pub fn rpc_error_unimplemented<T: AsRef<str>>(x: T) -> RPCError {
    error!("RPCError Unimplemented: {}", x.as_ref());
    RPCError::Unimplemented(x.as_ref().to_owned())
}

impl fmt::Display for RPCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RPCError::Timeout => write!(f, "[RPCError: Timeout]"),
            RPCError::InvalidFormat(s) => write!(f, "[RPCError: InvalidFormat({})]", s),
            RPCError::Unreachable(k) => write!(f, "[RPCError: Unreachable({})]", k),
            RPCError::Unimplemented(s) => write!(f, "[RPCError: Unimplemented({})]", s),
            RPCError::Protocol(s) => write!(f, "[RPCError: Protocol({})]", s),
            RPCError::Internal(s) => write!(f, "[RPCError: Internal({})]", s),
        }
    }
}

#[macro_export]
macro_rules! map_error_internal {
    ($x:expr) => {
        |_| rpc_error_internal($x)
    };
}
#[macro_export]
macro_rules! map_error_protocol {
    ($x:expr) => {
        |_| rpc_error_protocol($x)
    };
}
#[macro_export]
macro_rules! map_error_string {
    () => {
        |s| rpc_error_internal(&s)
    };
}
#[macro_export]
macro_rules! map_error_capnp_error {
    () => {
        |e| rpc_error_capnp_error(e)
    };
}

#[macro_export]
macro_rules! map_error_capnp_notinschema {
    () => {
        |e| rpc_error_capnp_notinschema(e)
    };
}

#[macro_export]
macro_rules! map_error_panic {
    () => {
        |_| panic!("oops")
    };
}

impl RPCProcessor {
    pub(super) fn get_rpc_message_debug_info<T: capnp::message::ReaderSegments>(
        &self,
        message: &capnp::message::Reader<T>,
    ) -> String {
        let operation = match message.get_root::<veilid_capnp::operation::Reader>() {
            Ok(v) => v,
            Err(e) => {
                return format!("invalid operation: {}", e);
            }
        };
        let op_id = operation.get_op_id();
        let detail = match operation.get_detail().which() {
            Ok(v) => v,
            Err(e) => {
                return format!("(operation detail not in schema: {})", e);
            }
        };
        format!(
            "#{} {}",
            op_id,
            self.get_rpc_operation_detail_debug_info(&detail)
        )
    }

    pub(super) fn get_rpc_operation_detail_debug_info(
        &self,
        detail: &veilid_capnp::operation::detail::WhichReader,
    ) -> String {
        match detail {
            veilid_capnp::operation::detail::StatusQ(_) => "StatusQ".to_owned(),
            veilid_capnp::operation::detail::StatusA(_) => "StatusA".to_owned(),
            veilid_capnp::operation::detail::ValidateDialInfo(_) => "ValidateDialInfo".to_owned(),
            veilid_capnp::operation::detail::FindNodeQ(_) => "FindNodeQ".to_owned(),
            veilid_capnp::operation::detail::FindNodeA(_) => "FindNodeA".to_owned(),
            veilid_capnp::operation::detail::Route(_) => "Route".to_owned(),
            veilid_capnp::operation::detail::NodeInfoUpdate(_) => "NodeInfoUpdate".to_owned(),
            veilid_capnp::operation::detail::GetValueQ(_) => "GetValueQ".to_owned(),
            veilid_capnp::operation::detail::GetValueA(_) => "GetValueA".to_owned(),
            veilid_capnp::operation::detail::SetValueQ(_) => "SetValueQ".to_owned(),
            veilid_capnp::operation::detail::SetValueA(_) => "SetValueA".to_owned(),
            veilid_capnp::operation::detail::WatchValueQ(_) => "WatchValueQ".to_owned(),
            veilid_capnp::operation::detail::WatchValueA(_) => "WatchValueA".to_owned(),
            veilid_capnp::operation::detail::ValueChanged(_) => "ValueChanged".to_owned(),
            veilid_capnp::operation::detail::SupplyBlockQ(_) => "SupplyBlockQ".to_owned(),
            veilid_capnp::operation::detail::SupplyBlockA(_) => "SupplyBlockA".to_owned(),
            veilid_capnp::operation::detail::FindBlockQ(_) => "FindBlockQ".to_owned(),
            veilid_capnp::operation::detail::FindBlockA(_) => "FindBlockA".to_owned(),
            veilid_capnp::operation::detail::Signal(_) => "Signal".to_owned(),
            veilid_capnp::operation::detail::ReturnReceipt(_) => "ReturnReceipt".to_owned(),
            veilid_capnp::operation::detail::StartTunnelQ(_) => "StartTunnelQ".to_owned(),
            veilid_capnp::operation::detail::StartTunnelA(_) => "StartTunnelA".to_owned(),
            veilid_capnp::operation::detail::CompleteTunnelQ(_) => "CompleteTunnelQ".to_owned(),
            veilid_capnp::operation::detail::CompleteTunnelA(_) => "CompleteTunnelA".to_owned(),
            veilid_capnp::operation::detail::CancelTunnelQ(_) => "CancelTunnelQ".to_owned(),
            veilid_capnp::operation::detail::CancelTunnelA(_) => "CancelTunnelA".to_owned(),
        }
    }
}
