use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
struct RPCOperation {
    op_id: u64,
    // index: u32,
    // is_q: bool,
    // wants_answer: bool,
    respond_to: RespondTo,
    detail: RPCOperationDetail,
}

impl RPCOperation {
    pub fn decode(
        operation_reader: &veilid_capnp::operation::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<Self, RPCError> {
        let op_id = operation_reader.get_op_id();

        let respond_to_reader = operation_reader.get_respond_to();
        let respond_to = RespondTo::decode(&respond_to_reader, sender_node_id)?;

        let detail_reader = operation_reader.get_detail();
        let detail = RPCOperationDetail::decode(&detail_reader, sender_node_id)?;

        Ok(RPCOperation {
            op_id,
            respond_to,
            detail,
        })
    }

    pub fn encode(&self, builder: &mut veilid_capnp::operation::Builder) -> Result<(), RPCError> {
        builder.set_op_id(self.op_id);
        let rt_builder = builder.init_respond_to();
        self.respond_to.encode(&mut rt_builder)?;
        let d_builder = builder.init_detail();
        self.detail.encode(&mut d_builder)?;
        Ok(())
    }
}

// let out = match which_reader {
//     veilid_capnp::operation::detail::StatusQ(_) => Self { name: "StatusQ", op_id, index: 0, is_q: true, wants_answer: true, respond_to },
//     veilid_capnp::operation::detail::StatusA(_) => Self { name: "StatusA", op_id,  index: 1, is_q: false, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::ValidateDialInfo(_) => Self { name: "ValidateDialInfo", op_id,  index: 2, is_q: true, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::FindNodeQ(_) => Self { name: "FindNodeQ", op_id,  index: 3, is_q: true, wants_answer: true, respond_to },
//     veilid_capnp::operation::detail::FindNodeA(_) => Self { name: "FindNodeA", op_id,  index: 4, is_q: false, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::Route(_) => Self { name: "Route", op_id,  index: 5, is_q: true, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::NodeInfoUpdate(_) => Self { name: "NodeInfoUpdate", op_id,  index: 6, is_q: true, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::GetValueQ(_) => Self { name: "GetValueQ", op_id,  index: 7, is_q: true, wants_answer: true, respond_to },
//     veilid_capnp::operation::detail::GetValueA(_) => Self { name: "GetValueA", op_id,  index: 8, is_q: false, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::SetValueQ(_) => Self { name: "SetValueQ", op_id,  index: 9, is_q: true, wants_answer: true, respond_to },
//     veilid_capnp::operation::detail::SetValueA(_) => Self { name: "SetValueA", op_id,  index: 10, is_q: false, wants_answer: false, respond_to},
//     veilid_capnp::operation::detail::WatchValueQ(_) => Self { name: "WatchValueQ", op_id,  index: 11, is_q: true, wants_answer: true, respond_to},
//     veilid_capnp::operation::detail::WatchValueA(_) => Self { name: "WatchValueA", op_id,  index: 12, is_q: false, wants_answer: false, respond_to},
//     veilid_capnp::operation::detail::ValueChanged(_) => Self { name: "ValueChanged", op_id,  index: 13, is_q: true, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::SupplyBlockQ(_) => Self { name: "SupplyBlockQ", op_id,  index: 14, is_q: true, wants_answer: true, respond_to },
//     veilid_capnp::operation::detail::SupplyBlockA(_) => Self { name: "SupplyBlockA", op_id,  index: 15, is_q: false, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::FindBlockQ(_) => Self { name: "FindBlockQ", op_id,  index: 16, is_q: true, wants_answer: true, respond_to},
//     veilid_capnp::operation::detail::FindBlockA(_) =>Self { name: "FindBlockA", op_id,  index: 17, is_q: false, wants_answer: false, respond_to},
//     veilid_capnp::operation::detail::Signal(_) => Self { name: "Signal", op_id,  index: 18, is_q: true, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::ReturnReceipt(_) => Self { name: "ReturnReceipt", op_id,  index: 19, is_q: true, wants_answer: false, respond_to},
//     veilid_capnp::operation::detail::StartTunnelQ(_) => Self { name: "StartTunnelQ", op_id,  index: 20, is_q: true, wants_answer: true, respond_to },
//     veilid_capnp::operation::detail::StartTunnelA(_) => Self { name: "StartTunnelA", op_id,  index: 21, is_q: false, wants_answer: false, respond_to },
//     veilid_capnp::operation::detail::CompleteTunnelQ(_) =>Self { name: "CompleteTunnelQ", op_id,  index: 22, is_q: true, wants_answer: true, respond_to},
//     veilid_capnp::operation::detail::CompleteTunnelA(_) => Self { name: "CompleteTunnelA", op_id,  index: 23, is_q: false, wants_answer: false, respond_to},
//     veilid_capnp::operation::detail::CancelTunnelQ(_) => Self { name: "CancelTunnelQ", op_id,  index: 24, is_q: true, wants_answer: true, respond_to},
//     veilid_capnp::operation::detail::CancelTunnelA(_) => Self { name: "CancelTunnelA", op_id,  index: 25, is_q: false, wants_answer: false, respond_to},
// };

// veilid_capnp::operation::detail::StatusQ(_) => Self { name: "StatusQ", op_id, index: 0, is_q: true, wants_answer: true, respond_to },
//             veilid_capnp::operation::detail::StatusA(_) => Self { name: "StatusA", op_id,  index: 1, is_q: false, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::ValidateDialInfo(_) => Self { name: "ValidateDialInfo", op_id,  index: 2, is_q: true, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::FindNodeQ(_) => Self { name: "FindNodeQ", op_id,  index: 3, is_q: true, wants_answer: true, respond_to },
//             veilid_capnp::operation::detail::FindNodeA(_) => Self { name: "FindNodeA", op_id,  index: 4, is_q: false, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::Route(_) => Self { name: "Route", op_id,  index: 5, is_q: true, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::NodeInfoUpdate(_) => Self { name: "NodeInfoUpdate", op_id,  index: 6, is_q: true, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::GetValueQ(_) => Self { name: "GetValueQ", op_id,  index: 7, is_q: true, wants_answer: true, respond_to },
//             veilid_capnp::operation::detail::GetValueA(_) => Self { name: "GetValueA", op_id,  index: 8, is_q: false, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::SetValueQ(_) => Self { name: "SetValueQ", op_id,  index: 9, is_q: true, wants_answer: true, respond_to },
//             veilid_capnp::operation::detail::SetValueA(_) => Self { name: "SetValueA", op_id,  index: 10, is_q: false, wants_answer: false, respond_to},
//             veilid_capnp::operation::detail::WatchValueQ(_) => Self { name: "WatchValueQ", op_id,  index: 11, is_q: true, wants_answer: true, respond_to},
//             veilid_capnp::operation::detail::WatchValueA(_) => Self { name: "WatchValueA", op_id,  index: 12, is_q: false, wants_answer: false, respond_to},
//             veilid_capnp::operation::detail::ValueChanged(_) => Self { name: "ValueChanged", op_id,  index: 13, is_q: true, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::SupplyBlockQ(_) => Self { name: "SupplyBlockQ", op_id,  index: 14, is_q: true, wants_answer: true, respond_to },
//             veilid_capnp::operation::detail::SupplyBlockA(_) => Self { name: "SupplyBlockA", op_id,  index: 15, is_q: false, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::FindBlockQ(_) => Self { name: "FindBlockQ", op_id,  index: 16, is_q: true, wants_answer: true, respond_to},
//             veilid_capnp::operation::detail::FindBlockA(_) =>Self { name: "FindBlockA", op_id,  index: 17, is_q: false, wants_answer: false, respond_to},
//             veilid_capnp::operation::detail::Signal(_) => Self { name: "Signal", op_id,  index: 18, is_q: true, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::ReturnReceipt(_) => Self { name: "ReturnReceipt", op_id,  index: 19, is_q: true, wants_answer: false, respond_to},
//             veilid_capnp::operation::detail::StartTunnelQ(_) => Self { name: "StartTunnelQ", op_id,  index: 20, is_q: true, wants_answer: true, respond_to },
//             veilid_capnp::operation::detail::StartTunnelA(_) => Self { name: "StartTunnelA", op_id,  index: 21, is_q: false, wants_answer: false, respond_to },
//             veilid_capnp::operation::detail::CompleteTunnelQ(_) =>Self { name: "CompleteTunnelQ", op_id,  index: 22, is_q: true, wants_answer: true, respond_to},
//             veilid_capnp::operation::detail::CompleteTunnelA(_) => Self { name: "CompleteTunnelA", op_id,  index: 23, is_q: false, wants_answer: false, respond_to},
//             veilid_capnp::operation::detail::CancelTunnelQ(_) => Self { name: "CancelTunnelQ", op_id,  index: 24, is_q: true, wants_answer: true, respond_to},
//             veilid_capnp::operation::detail::CancelTunnelA(_) => Self { name: "CancelTunnelA", op_id,  index: 25, is_q: false, wants_answer: false, respond_to},
