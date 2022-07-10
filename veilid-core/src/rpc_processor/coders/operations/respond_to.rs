use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub enum RespondTo {
    Sender(Option<SignedNodeInfo>),
    PrivateRoute(PrivateRoute),
}

impl RespondTo {
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::question::respond_to::Builder,
    ) -> Result<(), RPCError> {
        match self {
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
        reader: &veilid_capnp::question::respond_to::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<Self, RPCError> {
        let respond_to = match reader.which().map_err(RPCError::protocol)? {
            veilid_capnp::question::respond_to::Sender(()) => RespondTo::Sender(None),
            veilid_capnp::question::respond_to::SenderWithInfo(sender_ni_reader) => {
                let sender_ni_reader = sender_ni_reader.map_err(RPCError::protocol)?;
                let sni = decode_signed_node_info(&sender_ni_reader, sender_node_id, true)?;
                RespondTo::Sender(Some(sni))
            }
            veilid_capnp::question::respond_to::PrivateRoute(pr_reader) => {
                let pr_reader = pr_reader.map_err(RPCError::protocol)?;
                let pr = decode_private_route(&pr_reader)?;
                RespondTo::PrivateRoute(pr)
            }
        };
        Ok(respond_to)
    }
}
