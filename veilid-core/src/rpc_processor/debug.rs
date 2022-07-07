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

pub fn rpc_error_internal<T: ToString>(x: T) -> RPCError {
    let x = x.to_string();
    error!("RPCError Internal: {}", x);
    RPCError::Internal(x)
}
pub fn rpc_error_invalid_format<T: ToString>(x: T) -> RPCError {
    let x = x.to_string();
    error!("RPCError Invalid Format: {}", x);
    RPCError::InvalidFormat(x)
}
pub fn rpc_error_protocol<T: ToString>(x: T) -> RPCError {
    let x = x.to_string();
    error!("RPCError Protocol: {}", x);
    RPCError::Protocol(x)
}
pub fn rpc_error_capnp_error(e: capnp::Error) -> RPCError {
    error!("RPCError Protocol: capnp error: {}", &e.description);
    RPCError::Protocol(e.description)
}
pub fn rpc_error_capnp_notinschema(e: capnp::NotInSchema) -> RPCError {
    error!("RPCError Protocol: not in schema: {}", &e.0);
    RPCError::Protocol(format!("not in schema: {}", &e.0))
}
pub fn rpc_error_unimplemented<T: ToString>(x: T) -> RPCError {
    let x = x.to_string();
    error!("RPCError Unimplemented: {}", x);
    RPCError::Unimplemented(x)
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
