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
    pub(super) fn get_rpc_request_debug_info<T: capnp::message::ReaderSegments>(
        &self,
        dest: &Destination,
        message: &capnp::message::Reader<T>,
        safety_route_spec: &Option<&SafetyRouteSpec>,
    ) -> String {
        format!(
            "REQ->{:?}{} {}",
            dest,
            match safety_route_spec {
                None => "".to_owned(),
                Some(srs) => format!("[{:?}]", srs),
            },
            self.get_rpc_message_debug_info(message)
        )
    }
    pub(super) fn get_rpc_reply_debug_info<T: capnp::message::ReaderSegments>(
        &self,
        request_rpcreader: &RPCMessageReader,
        reply_msg: &capnp::message::Reader<T>,
        safety_route_spec: &Option<&SafetyRouteSpec>,
    ) -> String {
        let request_operation = match request_rpcreader
            .reader
            .get_root::<veilid_capnp::operation::Reader>()
        {
            Ok(v) => v,
            Err(e) => {
                return format!("invalid operation: {}", e);
            }
        };

        let respond_to = match request_operation.get_respond_to().which() {
            Ok(v) => v,
            Err(e) => {
                return format!("(respond_to not in schema: {:?})", e);
            }
        };
        let respond_to_str = match respond_to {
            veilid_capnp::operation::respond_to::None(_) => "(None)".to_owned(),
            veilid_capnp::operation::respond_to::Sender(_) => "Sender".to_owned(),
            veilid_capnp::operation::respond_to::SenderWithInfo(sni) => {
                let sni_reader = match sni {
                    Ok(snir) => snir,
                    Err(e) => {
                        return e.to_string();
                    }
                };
                let signed_node_info = match decode_signed_node_info(
                    &sni_reader,
                    &request_rpcreader.header.envelope.get_sender_id(),
                    true,
                ) {
                    Ok(ni) => ni,
                    Err(e) => {
                        return e.to_string();
                    }
                };
                format!("Sender({:?})", signed_node_info)
            }
            veilid_capnp::operation::respond_to::PrivateRoute(pr) => {
                let pr_reader = match pr {
                    Ok(prr) => prr,
                    Err(e) => {
                        return e.to_string();
                    }
                };
                let private_route = match decode_private_route(&pr_reader) {
                    Ok(pr) => pr,
                    Err(e) => {
                        return e.to_string();
                    }
                };
                format!("[PR:{:?}]", private_route)
            }
        };
        format!(
            "REPLY->{:?}{} {}",
            respond_to_str,
            match safety_route_spec {
                None => "".to_owned(),
                Some(srs) => format!("[SR:{:?}]", srs),
            },
            self.get_rpc_message_debug_info(reply_msg)
        )
    }

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

    #[allow(clippy::useless_format)]
    pub(super) fn get_rpc_operation_detail_debug_info(
        &self,
        detail: &veilid_capnp::operation::detail::WhichReader,
    ) -> String {
        match detail {
            veilid_capnp::operation::detail::StatusQ(_) => {
                format!("StatusQ")
            }
            veilid_capnp::operation::detail::StatusA(_) => {
                format!("StatusA")
            }
            veilid_capnp::operation::detail::ValidateDialInfo(_) => {
                format!("ValidateDialInfo")
            }
            veilid_capnp::operation::detail::FindNodeQ(d) => {
                let fnqr = match d {
                    Ok(fnqr) => fnqr,
                    Err(e) => {
                        return format!("(invalid detail: {})", e);
                    }
                };
                let nidr = match fnqr.get_node_id() {
                    Ok(nidr) => nidr,
                    Err(e) => {
                        return format!("(invalid node id: {})", e);
                    }
                };
                let node_id = decode_public_key(&nidr);
                format!("FindNodeQ: node_id={}", node_id.encode(),)
            }
            veilid_capnp::operation::detail::FindNodeA(d) => {
                let fnar = match d {
                    Ok(fnar) => fnar,
                    Err(e) => {
                        return format!("(invalid detail: {})", e);
                    }
                };

                let p_reader = match fnar.reborrow().get_peers() {
                    Ok(pr) => pr,
                    Err(e) => {
                        return format!("(invalid peers: {})", e);
                    }
                };
                let mut peers = Vec::<PeerInfo>::with_capacity(match p_reader.len().try_into() {
                    Ok(v) => v,
                    Err(e) => return format!("invalid peer count: {}", e),
                });
                for p in p_reader.iter() {
                    let peer_info = match decode_peer_info(&p, true) {
                        Ok(v) => v,
                        Err(e) => {
                            return format!("(unable to decode peer info: {})", e);
                        }
                    };
                    peers.push(peer_info);
                }

                format!("FindNodeA: peers={:#?}", peers)
            }
            veilid_capnp::operation::detail::Route(_) => {
                format!("Route")
            }
            veilid_capnp::operation::detail::NodeInfoUpdate(_) => {
                format!("NodeInfoUpdate")
            }
            veilid_capnp::operation::detail::GetValueQ(_) => {
                format!("GetValueQ")
            }
            veilid_capnp::operation::detail::GetValueA(_) => {
                format!("GetValueA")
            }
            veilid_capnp::operation::detail::SetValueQ(_) => {
                format!("SetValueQ")
            }
            veilid_capnp::operation::detail::SetValueA(_) => {
                format!("SetValueA")
            }
            veilid_capnp::operation::detail::WatchValueQ(_) => {
                format!("WatchValueQ")
            }
            veilid_capnp::operation::detail::WatchValueA(_) => {
                format!("WatchValueA")
            }
            veilid_capnp::operation::detail::ValueChanged(_) => {
                format!("ValueChanged")
            }
            veilid_capnp::operation::detail::SupplyBlockQ(_) => {
                format!("SupplyBlockQ")
            }
            veilid_capnp::operation::detail::SupplyBlockA(_) => {
                format!("SupplyBlockA")
            }
            veilid_capnp::operation::detail::FindBlockQ(_) => {
                format!("FindBlockQ")
            }
            veilid_capnp::operation::detail::FindBlockA(_) => {
                format!("FindBlockA")
            }
            veilid_capnp::operation::detail::Signal(_) => {
                format!("Signal")
            }
            veilid_capnp::operation::detail::ReturnReceipt(_) => {
                format!("ReturnReceipt")
            }
            veilid_capnp::operation::detail::StartTunnelQ(_) => {
                format!("StartTunnelQ")
            }
            veilid_capnp::operation::detail::StartTunnelA(_) => {
                format!("StartTunnelA")
            }
            veilid_capnp::operation::detail::CompleteTunnelQ(_) => {
                format!("CompleteTunnelQ")
            }
            veilid_capnp::operation::detail::CompleteTunnelA(_) => {
                format!("CompleteTunnelA")
            }
            veilid_capnp::operation::detail::CancelTunnelQ(_) => {
                format!("CancelTunnelQ")
            }
            veilid_capnp::operation::detail::CancelTunnelA(_) => {
                format!("CancelTunnelA")
            }
        }
    }
}
