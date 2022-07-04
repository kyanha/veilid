use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub enum RPCOperationKind {
    Question(RPCQuestion),
    Statement(RPCStatement),
    Answer(RPCAnswer),
}

impl RPCOperationKind {
    pub fn desc(&self) -> &'static str {
        match self {
            RPCOperationKind::Question(q) => q.desc(),
            RPCOperationKind::Statement(s) => s.desc(),
            RPCOperationKind::Answer(a) => a.desc(),
        }
    }

    pub fn decode(
        kind_reader: &veilid_capnp::operation::kind::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<Self, RPCError> {
        let which_reader = kind_reader
            .which()
            .map_err(map_error_capnp_notinschema!())?;
        let out = match which_reader {
            veilid_capnp::operation::kind::Which::Question(r) => {
                let q_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCQuestion::decode(&q_reader, sender_node_id)?;
                RPCOperationKind::Question(out)
            }
            veilid_capnp::operation::kind::Which::Statement(r) => {
                let q_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCStatement::decode(&q_reader, sender_node_id)?;
                RPCOperationKind::Statement(out)
            }
            veilid_capnp::operation::kind::Which::Answer(r) => {
                let q_reader = r.map_err(map_error_capnp_notinschema!())?;
                let out = RPCAnswer::decode(&q_reader)?;
                RPCOperationKind::Answer(out)
            }
        };

        Ok(out)
    }

    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation::kind::Builder,
    ) -> Result<(), RPCError> {
        match self {
            RPCOperationKind::Question(k) => k.encode(&mut builder.init_question()),
            RPCOperationKind::Statement(k) => k.encode(&mut builder.init_statement()),
            RPCOperationKind::Answer(k) => k.encode(&mut builder.init_answer()),
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RPCOperation {
    op_id: u64,
    kind: RPCOperationKind,
}

impl RPCOperation {
    pub fn new_question(question: RPCQuestion) -> Self {
        Self {
            op_id: intf::get_random_u64(),
            kind: RPCOperationKind::Question(question),
        }
    }
    pub fn new_statement(statement: RPCStatement) -> Self {
        Self {
            op_id: intf::get_random_u64(),
            kind: RPCOperationKind::Statement(statement),
        }
    }

    pub fn new_answer(request: &RPCOperation, answer: RPCAnswer) -> Self {
        Self {
            op_id: request.op_id,
            kind: RPCOperationKind::Answer(answer),
        }
    }

    pub fn op_id(&self) -> u64 {
        self.op_id
    }

    pub fn kind(&self) -> &RPCOperationKind {
        &self.kind
    }

    pub fn into_kind(&self) -> RPCOperationKind {
        self.kind
    }

    pub fn decode(
        operation_reader: &veilid_capnp::operation::Reader,
        sender_node_id: &DHTKey,
    ) -> Result<Self, RPCError> {
        let op_id = operation_reader.get_op_id();

        let kind_reader = operation_reader.get_kind();
        let kind = RPCOperationKind::decode(&kind_reader, sender_node_id)?;

        Ok(RPCOperation { op_id, kind })
    }

    pub fn encode(&self, builder: &mut veilid_capnp::operation::Builder) -> Result<(), RPCError> {
        builder.set_op_id(self.op_id);
        let k_builder = builder.init_kind();
        self.kind.encode(&mut k_builder)?;
        Ok(())
    }
}
