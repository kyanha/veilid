use super::*;

#[derive(ThisError, Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
#[must_use]
pub enum RPCError {
    #[error("[RPCError: Unreachable({0})]")]
    Unreachable(DHTKey),
    #[error("[RPCError: Unimplemented({0})]")]
    Unimplemented(String),
    #[error("[RPCError: InvalidFormat({0})]")]
    InvalidFormat(String),
    #[error("[RPCError: Protocol({0})]")]
    Protocol(String),
    #[error("[RPCError: Internal({0})]")]
    Internal(String),
    #[error("[RPCError: Network({0})]")]
    Network(String),
}

impl RPCError {
    pub fn unreachable(key: DHTKey) -> Self {
        Self::Unreachable(key)
    }
    pub fn unimplemented<X: ToString>(x: X) -> Self {
        Self::Unimplemented(x.to_string())
    }
    pub fn invalid_format<X: ToString>(x: X) -> Self {
        Self::InvalidFormat(x.to_string())
    }
    pub fn map_invalid_format<M: ToString, X: ToString>(message: M) -> impl FnOnce(X) -> Self {
        move |x| Self::InvalidFormat(format!("{}: {}", message.to_string(), x.to_string()))
    }
    pub fn protocol<X: ToString>(x: X) -> Self {
        Self::Protocol(x.to_string())
    }
    pub fn map_protocol<M: ToString, X: ToString>(message: M) -> impl FnOnce(X) -> Self {
        move |x| Self::Protocol(format!("{}: {}", message.to_string(), x.to_string()))
    }
    pub fn internal<X: ToString>(x: X) -> Self {
        Self::Internal(x.to_string())
    }
    pub fn map_internal<M: ToString, X: ToString>(message: M) -> impl FnOnce(X) -> Self {
        move |x| Self::Internal(format!("{}: {}", message.to_string(), x.to_string()))
    }
    pub fn else_internal<M: ToString>(message: M) -> impl FnOnce() -> Self {
        move || Self::Internal(format!("{}", message.to_string()))
    }
    pub fn network<X: ToString>(x: X) -> Self {
        Self::Network(x.to_string())
    }
    pub fn map_network<M: ToString, X: ToString>(message: M) -> impl FnOnce(X) -> Self {
        move |x| Self::Network(format!("{}: {}", message.to_string(), x.to_string()))
    }
}
