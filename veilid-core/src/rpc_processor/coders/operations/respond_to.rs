use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub enum RespondTo {
    None,
    Sender(Option<SignedNodeInfo>),
    PrivateRoute(PrivateRoute),
}

impl RespondTo {
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation::respond_to::Builder,
    ) -> Result<(), RPCError> {
        match self {
            Self::None => {
                builder.set_none(());
            }
            Self::Sender(Some(sni)) => {
                let mut sni_builder = builder.reborrow().init_sender_with_info();
                encode_signed_node_info(sni, &mut sni_builder)?;
            }
            Self::Sender(None) => {
                builder.reborrow().set_sender(());
            }
            Self::PrivateRoute(pr) => {
                let mut pr_builder = builder.reborrow().init_private_route();
                encode_private_route(pr, &mut pr_builder)?;
            }
        };
        Ok(())
    }

    pub fn decode(
        reader: &veilid_capnp::operation::respond_to::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<Self, RPCError> {
        let respond_to = match reader.which().map_err(map_error_capnp_notinschema!())? {
            veilid_capnp::operation::respond_to::None(_) => RespondTo::None,
            veilid_capnp::operation::respond_to::Sender(_) => RespondTo::Sender(None),
            veilid_capnp::operation::respond_to::SenderWithInfo(Ok(sender_ni_reader)) => {
                let sni = decode_signed_node_info(&sender_ni_reader, sender_node_id, true)?;
                RespondTo::Sender(Some(sni))
            }
            veilid_capnp::operation::respond_to::SenderWithInfo(Err(e)) => {
                return Err(rpc_error_protocol(format!(
                    "invalid signed node info: {}",
                    e
                )))
            }
            veilid_capnp::operation::respond_to::PrivateRoute(Ok(pr_reader)) => {
                let pr = decode_private_route(&pr_reader)?;
                RespondTo::PrivateRoute(pr)
            }
            veilid_capnp::operation::respond_to::PrivateRoute(Err(e)) => {
                return Err(rpc_error_protocol(format!("invalid private route: {}", e)));
            }
        };
        Ok(respond_to)
    }
}
