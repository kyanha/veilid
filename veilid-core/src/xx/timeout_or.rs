use super::*;
use cfg_if::*;
use core::fmt::{Debug, Display};
use core::result::Result;
use std::error::Error;
use std::io;

#[derive(ThisError, Debug, Clone, Copy, Eq, PartialEq)]
#[error("Timeout")]
pub struct TimeoutError();

impl TimeoutError {
    pub fn to_io(self) -> io::Error {
        io::Error::new(io::ErrorKind::TimedOut, self)
    }
}

cfg_if! {
    if #[cfg(feature="rt-async-std")] {
        impl From<async_std::future::TimeoutError> for TimeoutError {
            fn from(_: async_std::future::TimeoutError) -> Self {
                Self()
            }
        }
    } else if #[cfg(feature="rt-tokio")] {
        impl From<tokio::time::error::Elapsed> for TimeoutError {
            fn from(_: tokio::time::error::Elapsed) -> Self {
                Self()
            }
        }
    }
}

//////////////////////////////////////////////////////////////////

pub enum TimeoutOr<T> {
    Timeout,
    Value(T),
}

impl<T> TimeoutOr<T> {
    pub fn timeout() -> Self {
        Self::Timeout
    }
    pub fn value(value: T) -> Self {
        Self::Value(value)
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }

    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value(_))
    }

    pub fn ok(self) -> Result<T, TimeoutError> {
        match self {
            Self::Timeout => Err(TimeoutError {}),
            Self::Value(v) => Ok(v),
        }
    }
}

impl<T> From<TimeoutOr<T>> for Option<T> {
    fn from(t: TimeoutOr<T>) -> Self {
        match t {
            TimeoutOr::<T>::Timeout => None,
            TimeoutOr::<T>::Value(v) => Some(v),
        }
    }
}

impl<T: Clone> Clone for TimeoutOr<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Timeout => Self::Timeout,
            Self::Value(t) => Self::Value(t.clone()),
        }
    }
}
impl<T: Debug> Debug for TimeoutOr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Timeout => write!(f, "Timeout"),
            Self::Value(arg0) => f.debug_tuple("Value").field(arg0).finish(),
        }
    }
}
impl<T: Display> Display for TimeoutOr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, ""),
            Self::Value(arg0) => write!(f, "{}", arg0),
        }
    }
}
impl<T: Debug + Display> Error for TimeoutOr<T> {}
