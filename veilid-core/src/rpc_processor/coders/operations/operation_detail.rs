use super::*;
use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub enum RPCOperationDetail {
    StatusQ(RPCOperationStatusQ),
    StatusA(RPCOperationStatusA),
    ValidateDialInfo(RPCOperationValidateDialInfo),
    FindNodeQ(RPCOperationFindNodeQ),
    FindNodeA(RPCOperationFindNodeA),
    Route(RPCOperationRoute),
    NodeInfoUpdate(RPCOperationNodeInfoUpdate),
    GetValueQ(RPCOperationGetValueQ),
    GetValueA(RPCOperationGetValueA),
    SetValueQ(RPCOperationSetValueQ),
    SetValueA(RPCOperationSetValueA),
    WatchValueQ(RPCOperationWatchValueQ),
    WatchValueA(RPCOperationWatchValueA),
    ValueChanged(RPCOperationValueChanged),
    SupplyBlockQ(RPCOperationSupplyBlockQ),
    SupplyBlockA(RPCOperationSupplyBlockA),
    FindBlockQ(RPCOperationFindBlockQ),
    FindBlockA(RPCOperationFindBlockA),
    Signal(RPCOperationSignal),
    ReturnReceipt(RPCOperationReturnReceipt),
    StartTunnelQ(RPCOperationStartTunnelQ),
    StartTunnelA(RPCOperationStartTunnelA),
    CompleteTunnelQ(RPCOperationCompleteTunnelQ),
    CompleteTunnelA(RPCOperationCompleteTunnelA),
    CancelTunnelQ(CancelTunnelQ),
    CancelTunnelA(CancelTunnelA),
}

impl RPCOperationDetail {
    pub fn decode(
        reader: &veilid_capnp::operation::detail::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<RPCOperationDetail, RPCError> {
        let which_reader = reader.which().map_err(map_error_capnp_notinschema!())?;
        let out = match which_reader {
            veilid_capnp::operation::detail::StatusQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationStatusQ::decode(&op_reader)?;
                RPCOperationDetail::StatusQ(out)
            }
            veilid_capnp::operation::detail::StatusA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationStatusA::decode(&op_reader)?;
                RPCOperationDetail::StatusA(out)
            }
            veilid_capnp::operation::detail::ValidateDialInfo(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationValidateDialInfo::decode(&op_reader)?;
                RPCOperationDetail::ValidateDialInfo(out)
            }
            veilid_capnp::operation::detail::FindNodeQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationFindNodeQ::decode(&op_reader)?;
                RPCOperationDetail::FindNodeQ(out)
            }
            veilid_capnp::operation::detail::FindNodeA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationFindNodeA::decode(&op_reader)?;
                RPCOperationDetail::FindNodeA(out)
            }
            veilid_capnp::operation::detail::Route(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationRoute::decode(&op_reader)?;
                RPCOperationDetail::Route(out)
            }
            veilid_capnp::operation::detail::NodeInfoUpdate(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationNodeInfoUpdate::decode(&op_reader, sender_node_id)?;
                RPCOperationDetail::NodeInfoUpdate(out)
            }
            veilid_capnp::operation::detail::GetValueQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationGetValueQ::decode(&op_reader)?;
                RPCOperationDetail::GetValueQ(out)
            }
            veilid_capnp::operation::detail::GetValueA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationGetValueA::decode(&op_reader)?;
                RPCOperationDetail::GetValueA(out)
            }
            veilid_capnp::operation::detail::SetValueQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationSetValueQ::decode(&op_reader)?;
                RPCOperationDetail::SetValueQ(out)
            }
            veilid_capnp::operation::detail::SetValueA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationSetValueA::decode(&op_reader)?;
                RPCOperationDetail::SetValueA(out)
            }
            veilid_capnp::operation::detail::WatchValueQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationWatchValueQ::decode(&op_reader)?;
                RPCOperationDetail::WatchValueQ(out)
            }
            veilid_capnp::operation::detail::WatchValueA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationWatchValueA::decode(&op_reader)?;
                RPCOperationDetail::WatchValueA(out)
            }
            veilid_capnp::operation::detail::ValueChanged(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationValueChanged::decode(&op_reader)?;
                RPCOperationDetail::ValueChanged(out)
            }
            veilid_capnp::operation::detail::SupplyBlockQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationSupplyBlockQ::decode(&op_reader)?;
                RPCOperationDetail::SupplyBlockQ(out)
            }
            veilid_capnp::operation::detail::SupplyBlockA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationSupplyBlockA::decode(&op_reader)?;
                RPCOperationDetail::SupplyBlockA(out)
            }
            veilid_capnp::operation::detail::FindBlockQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationFindBlockQ::decode(&op_reader)?;
                RPCOperationDetail::FindBlockQ(out)
            }
            veilid_capnp::operation::detail::FindBlockA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationFindBlockA::decode(&op_reader)?;
                RPCOperationDetail::FindBlockA(out)
            }
            veilid_capnp::operation::detail::Signal(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationSignal::decode(&op_reader)?;
                RPCOperationDetail::Signal(out)
            }
            veilid_capnp::operation::detail::ReturnReceipt(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationReturnReceipt::decode(&op_reader)?;
                RPCOperationDetail::ReturnReceipt(out)
            }
            veilid_capnp::operation::detail::StartTunnelQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationStartTunnelQ::decode(&op_reader)?;
                RPCOperationDetail::StartTunnelQ(out)
            }
            veilid_capnp::operation::detail::StartTunnelA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationStartTunnelA::decode(&op_reader)?;
                RPCOperationDetail::StartTunnelA(out)
            }
            veilid_capnp::operation::detail::CompleteTunnelQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationCompleteTunnelQ::decode(&op_reader)?;
                RPCOperationDetail::CompleteTunnelQ(out)
            }
            veilid_capnp::operation::detail::CompleteTunnelA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationCompleteTunnelA::decode(&op_reader)?;
                RPCOperationDetail::CompleteTunnelA(out)
            }
            veilid_capnp::operation::detail::CancelTunnelQ(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationCancelTunnelQ::decode(&op_reader)?;
                RPCOperationDetail::CancelTunnelQ(out)
            }
            veilid_capnp::operation::detail::CancelTunnelA(r) => {
                let op_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCOperationCancelTunnelA::decode(&op_reader)?;
                RPCOperationDetail::CancelTunnelA(out)
            }
        };
        Ok(out)
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation::detail::Builder,
    ) -> Result<(), RPCError> {
        match self {
            RPCOperationDetail::StatusQ(d) => d.encode(&mut builder.init_status_q()),
            RPCOperationDetail::StatusA(d) => d.encode(&mut builder.init_status_a()),
            RPCOperationDetail::ValidateDialInfo(d) => {
                d.encode(&mut builder.init_validate_dial_info())
            }
            RPCOperationDetail::FindNodeQ(d) => d.encode(&mut builder.init_find_node_q()),
            RPCOperationDetail::FindNodeA(d) => d.encode(&mut builder.init_find_node_a()),
            RPCOperationDetail::Route(d) => d.encode(&mut builder.init_route()),
            RPCOperationDetail::NodeInfoUpdate(d) => d.encode(&mut builder.init_node_info_update()),
            RPCOperationDetail::GetValueQ(d) => d.encode(&mut builder.init_get_value_q()),
            RPCOperationDetail::GetValueA(d) => d.encode(&mut builder.init_get_value_a()),
            RPCOperationDetail::SetValueQ(d) => d.encode(&mut builder.init_set_value_q()),
            RPCOperationDetail::SetValueA(d) => d.encode(&mut builder.init_set_value_a()),
            RPCOperationDetail::WatchValueQ(d) => d.encode(&mut builder.init_watch_value_q()),
            RPCOperationDetail::WatchValueA(d) => d.encode(&mut builder.init_watch_value_a()),
            RPCOperationDetail::ValueChanged(d) => d.encode(&mut builder.init_value_changed()),
            RPCOperationDetail::SupplyBlockQ(d) => d.encode(&mut builder.init_supply_block_q()),
            RPCOperationDetail::SupplyBlockA(d) => d.encode(&mut builder.init_supply_block_a()),
            RPCOperationDetail::FindBlockQ(d) => d.encode(&mut builder.init_find_block_q()),
            RPCOperationDetail::FindBlockA(d) => d.encode(&mut builder.init_find_block_a()),
            RPCOperationDetail::Signal(d) => d.encode(&mut builder.init_signal()),
            RPCOperationDetail::ReturnReceipt(d) => d.encode(&mut builder.init_return_receipt()),
            RPCOperationDetail::StartTunnelQ(d) => d.encode(&mut builder.init_start_tunnel_q()),
            RPCOperationDetail::StartTunnelA(d) => d.encode(&mut builder.init_start_tunnel_a()),
            RPCOperationDetail::CompleteTunnelQ(d) => {
                d.encode(&mut builder.init_complete_tunnel_q())
            }
            RPCOperationDetail::CompleteTunnelA(d) => {
                d.encode(&mut builder.init_complete_tunnel_a())
            }
            RPCOperationDetail::CancelTunnelQ(d) => d.encode(&mut builder.init_cancel_tunnel_q()),
            RPCOperationDetail::CancelTunnelA(d) => d.encode(&mut builder.init_cancel_tunnel_a()),
        }
    }
}
