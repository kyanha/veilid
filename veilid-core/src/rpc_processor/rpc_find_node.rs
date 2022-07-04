use super::*;

impl RPCProcessor {
    // Send FindNodeQ RPC request, receive FindNodeA answer
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_find_node(
        self,
        dest: Destination,
        key: DHTKey,
        safety_route: Option<&SafetyRouteSpec>,
        respond_to: RespondTo,
    ) -> Result<Answer<Vec<PeerInfo>>, RPCError> {
        let find_node_q = RPCOperationFindNodeQ { node_id: key };
        let question = RPCQuestion::new(respond_to, RPCQuestionDetail::FindNodeQ(find_node_q));

        // Send the find_node request
        let waitable_reply = self.question(dest, question, safety_route).await?;

        // Wait for reply
        let (msg, latency) = self.wait_for_reply(waitable_reply).await?;

        // Get the right answer type
        let find_node_a = match msg.operation.into_kind() {
            RPCOperationKind::Answer(a) => match a.into_detail() {
                RPCAnswerDetail::FindNodeA(a) => a,
                _ => return Err(rpc_error_invalid_format("not a find_node answer")),
            },
            _ => return Err(rpc_error_invalid_format("not an answer")),
        };

        // Verify peers are in the correct peer scope
        for peer_info in &find_node_a.peers {
            if !self.filter_peer_scope(&peer_info.signed_node_info.node_info) {
                return Err(rpc_error_invalid_format(
                    "find_node response has invalid peer scope",
                ));
            }
        }

        Ok(Answer::new(latency, find_node_a.peers))
    }

    pub(crate) async fn process_find_node_q(&self, msg: RPCMessage) -> Result<(), RPCError> {
        // Get the question
        let find_node_q = match msg.operation.kind() {
            RPCOperationKind::Question(q) => match q.detail() {
                RPCQuestionDetail::FindNodeQ(q) => q,
                _ => panic!("not a status question"),
            },
            _ => panic!("not a question"),
        };

        // add node information for the requesting node to our routing table
        let routing_table = self.routing_table();

        // find N nodes closest to the target node in our routing table
        let own_peer_info = routing_table.get_own_peer_info();
        let own_peer_info_is_valid = own_peer_info.signed_node_info.is_valid();

        let closest_nodes = routing_table.find_closest_nodes(
            find_node_q.node_id,
            // filter
            Some(move |_k, v| {
                RoutingTable::filter_has_valid_signed_node_info(v, own_peer_info_is_valid)
            }),
            // transform
            move |k, v| RoutingTable::transform_to_peer_info(k, v, &own_peer_info),
        );

        // Make status answer
        let find_node_a = RPCOperationFindNodeA {
            peers: closest_nodes,
        };

        // Send status answer
        self.answer(
            msg,
            RPCAnswer::new(RPCAnswerDetail::FindNodeA(find_node_a)),
            None,
        )
        .await
    }
}
