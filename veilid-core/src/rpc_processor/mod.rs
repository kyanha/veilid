mod coders;
mod debug;
mod private_route;

pub use debug::*;
pub use private_route::*;

use crate::dht::*;
use crate::intf::*;
use crate::xx::*;
use crate::*;
use capnp::message::ReaderSegments;
use coders::*;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use network_manager::*;
use receipt_manager::*;
use routing_table::*;

/////////////////////////////////////////////////////////////////////

type OperationId = u64;

#[derive(Debug, Clone)]
pub enum Destination {
    Direct(NodeRef),            // Send to node
    Relay(NodeRef, DHTKey),     // Send to node for relay purposes
    PrivateRoute(PrivateRoute), // Send to private route
}

#[derive(Debug, Clone)]
pub enum RespondTo {
    None,
    Sender(Option<NodeInfo>),
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
            Self::Sender(Some(ni)) => {
                let mut ni_builder = builder.reborrow().init_sender();
                encode_node_info(ni, &mut ni_builder)?;
            }
            Self::Sender(None) => {
                builder.reborrow().init_sender();
            }
            Self::PrivateRoute(pr) => {
                let mut pr_builder = builder.reborrow().init_private_route();
                encode_private_route(pr, &mut pr_builder)?;
            }
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RPCMessageHeader {
    timestamp: u64,
    envelope: envelope::Envelope,
    body_len: u64,
    peer_noderef: NodeRef, // ensures node doesn't get evicted from routing table until we're done with it
}

#[derive(Debug, Clone)]
struct RPCMessageData {
    contents: Vec<u8>, // rpc messages must be a canonicalized single segment
}

impl ReaderSegments for RPCMessageData {
    fn get_segment(&self, idx: u32) -> Option<&[u8]> {
        if idx > 0 {
            None
        } else {
            Some(self.contents.as_slice())
        }
    }
}

#[derive(Debug)]
struct RPCMessage {
    header: RPCMessageHeader,
    data: RPCMessageData,
}

struct RPCMessageReader {
    header: RPCMessageHeader,
    reader: capnp::message::Reader<RPCMessageData>,
    opt_sender_nr: Option<NodeRef>,
}

fn builder_to_vec<'a, T>(builder: capnp::message::Builder<T>) -> Result<Vec<u8>, RPCError>
where
    T: capnp::message::Allocator + 'a,
{
    let wordvec = builder
        .into_reader()
        .canonicalize()
        .map_err(map_error_capnp_error!())
        .map_err(logthru_rpc!())?;
    Ok(capnp::Word::words_to_bytes(wordvec.as_slice()).to_vec())
}
fn reader_to_vec<'a, T>(reader: capnp::message::Reader<T>) -> Result<Vec<u8>, RPCError>
where
    T: capnp::message::ReaderSegments + 'a,
{
    let wordvec = reader
        .canonicalize()
        .map_err(map_error_capnp_error!())
        .map_err(logthru_rpc!())?;
    Ok(capnp::Word::words_to_bytes(wordvec.as_slice()).to_vec())
}

#[derive(Debug)]
struct WaitableReply {
    op_id: OperationId,
    eventual: EventualValue<RPCMessageReader>,
    timeout: u64,
    node_ref: NodeRef,
    send_ts: u64,
    send_data_kind: SendDataKind,
}

/////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct InfoAnswer {
    pub latency: u64,
    pub node_status: NodeStatus,
    pub sender_info: SenderInfo,
}

#[derive(Clone, Debug, Default)]
pub struct FindNodeAnswer {
    pub latency: u64,         // how long it took to get this answer
    pub peers: Vec<PeerInfo>, // the list of closer peers
}

/////////////////////////////////////////////////////////////////////

pub struct RPCProcessorInner {
    network_manager: NetworkManager,
    routing_table: RoutingTable,
    node_id: key::DHTKey,
    node_id_secret: key::DHTKeySecret,
    send_channel: Option<flume::Sender<RPCMessage>>,
    timeout: u64,
    max_route_hop_count: usize,
    waiting_rpc_table: BTreeMap<OperationId, EventualValue<RPCMessageReader>>,
    worker_join_handles: Vec<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct RPCProcessor {
    crypto: Crypto,
    config: VeilidConfig,
    enable_local_peer_scope: bool,
    inner: Arc<Mutex<RPCProcessorInner>>,
}

impl RPCProcessor {
    fn new_inner(network_manager: NetworkManager) -> RPCProcessorInner {
        RPCProcessorInner {
            network_manager: network_manager.clone(),
            routing_table: network_manager.routing_table(),
            node_id: key::DHTKey::default(),
            node_id_secret: key::DHTKeySecret::default(),
            send_channel: None,
            timeout: 10000000,
            max_route_hop_count: 7,
            waiting_rpc_table: BTreeMap::new(),
            worker_join_handles: Vec::new(),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            crypto: network_manager.crypto(),
            config: network_manager.config(),
            enable_local_peer_scope: network_manager
                .config()
                .get()
                .network
                .enable_local_peer_scope,
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
        }
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }

    pub fn routing_table(&self) -> RoutingTable {
        self.inner.lock().routing_table.clone()
    }

    pub fn node_id(&self) -> key::DHTKey {
        self.inner.lock().node_id
    }

    pub fn node_id_secret(&self) -> key::DHTKeySecret {
        self.inner.lock().node_id_secret
    }

    //////////////////////////////////////////////////////////////////////

    fn get_next_op_id(&self) -> OperationId {
        get_random_u64()
    }

    fn filter_peer_scope(&self, node_info: &NodeInfo) -> bool {
        // if local peer scope is enabled, then don't reject any peer info
        if self.enable_local_peer_scope {
            return true;
        }

        // reject attempts to include non-public addresses in results
        for did in &node_info.dial_info_detail_list {
            if !did.dial_info.is_global() {
                // non-public address causes rejection
                return false;
            }
        }
        if let Some(rpi) = &node_info.relay_peer_info {
            for did in &rpi.node_info.dial_info_detail_list {
                if !did.dial_info.is_global() {
                    // non-public address causes rejection
                    return false;
                }
            }
        }
        true
    }

    //////////////////////////////////////////////////////////////////////

    // Search the DHT for a single node closest to a key and add it to the routing table and return the node reference
    pub async fn search_dht_single_key(
        &self,
        node_id: key::DHTKey,
        _count: u32,
        _fanout: u32,
        _timeout: Option<u64>,
    ) -> Result<NodeRef, RPCError> {
        let routing_table = self.routing_table();

        // xxx find node but stop if we find the exact node we want
        // xxx return whatever node is closest after the timeout
        Err(rpc_error_unimplemented("search_dht_single_key")).map_err(logthru_rpc!(error))
    }

    // Search the DHT for the 'count' closest nodes to a key, adding them all to the routing table if they are not there and returning their node references
    pub async fn search_dht_multi_key(
        &self,
        _node_id: key::DHTKey,
        _count: u32,
        _fanout: u32,
        _timeout: Option<u64>,
    ) -> Result<Vec<NodeRef>, RPCError> {
        // xxx return closest nodes after the timeout
        Err(rpc_error_unimplemented("search_dht_multi_key")).map_err(logthru_rpc!(error))
    }

    // Search the DHT for a specific node corresponding to a key unless we have that node in our routing table already, and return the node reference
    // Note: This routine can possible be recursive, hence the SystemPinBoxFuture async form
    pub fn resolve_node(
        &self,
        node_id: key::DHTKey,
    ) -> SystemPinBoxFuture<Result<NodeRef, RPCError>> {
        let this = self.clone();
        Box::pin(async move {
            let routing_table = this.routing_table();

            // First see if we have the node in our routing table already
            if let Some(nr) = routing_table.lookup_node_ref(node_id) {
                // ensure we have some dial info for the entry already,
                // if not, we should do the find_node anyway
                if nr.has_any_dial_info() {
                    return Ok(nr);
                }
            }

            // If nobody knows where this node is, ask the DHT for it
            let (count, fanout, timeout) = {
                let c = this.config.get();
                (
                    c.network.dht.resolve_node_count,
                    c.network.dht.resolve_node_fanout,
                    c.network.dht.resolve_node_timeout_ms.map(ms_to_us),
                )
            };

            let nr = this
                .search_dht_single_key(node_id, count, fanout, timeout)
                .await?;

            if nr.node_id() != node_id {
                // found a close node, but not exact within our configured resolve_node timeout
                return Err(RPCError::Timeout).map_err(logthru_rpc!());
            }

            Ok(nr)
        })
    }

    // set up wait for reply
    fn add_op_id_waiter(&self, op_id: OperationId) -> EventualValue<RPCMessageReader> {
        let mut inner = self.inner.lock();
        let e = EventualValue::new();
        inner.waiting_rpc_table.insert(op_id, e.clone());
        e
    }

    // remove wait for reply
    fn cancel_op_id_waiter(&self, op_id: OperationId) {
        let mut inner = self.inner.lock();
        inner.waiting_rpc_table.remove(&op_id);
    }

    // complete the reply
    async fn complete_op_id_waiter(
        &self,
        op_id: OperationId,
        rpcreader: RPCMessageReader,
    ) -> Result<(), RPCError> {
        let eventual = {
            let mut inner = self.inner.lock();
            inner
                .waiting_rpc_table
                .remove(&op_id)
                .ok_or_else(|| rpc_error_internal("Unmatched operation id"))?
        };
        eventual.resolve(rpcreader).await;
        Ok(())
    }

    // wait for reply
    async fn do_wait_for_reply(
        &self,
        waitable_reply: &WaitableReply,
    ) -> Result<(RPCMessageReader, u64), RPCError> {
        let timeout_ms = u32::try_from(waitable_reply.timeout / 1000u64)
            .map_err(map_error_internal!("invalid timeout"))?;
        // wait for eventualvalue
        let start_ts = get_timestamp();
        timeout(timeout_ms, waitable_reply.eventual.instance())
            .await
            .map_err(|_| RPCError::Timeout)?;
        match waitable_reply.eventual.take_value() {
            None => panic!("there should be a reply value but there wasn't"),
            Some(rpcreader) => {
                let end_ts = get_timestamp();
                Ok((rpcreader, end_ts - start_ts))
            }
        }
    }
    async fn wait_for_reply(
        &self,
        waitable_reply: WaitableReply,
    ) -> Result<(RPCMessageReader, u64), RPCError> {
        let out = self.do_wait_for_reply(&waitable_reply).await;
        match &out {
            Err(_) => {
                self.cancel_op_id_waiter(waitable_reply.op_id);

                self.routing_table()
                    .stats_question_lost(waitable_reply.node_ref.clone(), waitable_reply.send_ts);
            }
            Ok((rpcreader, _)) => {
                // Note that we definitely received this node info since we got a reply
                waitable_reply.node_ref.set_seen_our_node_info();

                // Reply received
                let recv_ts = get_timestamp();
                self.routing_table().stats_answer_rcvd(
                    waitable_reply.node_ref,
                    waitable_reply.send_ts,
                    recv_ts,
                    rpcreader.header.body_len,
                )
            }
        };

        out
    }

    // Issue a request over the network, possibly using an anonymized route
    async fn request<T: capnp::message::ReaderSegments>(
        &self,
        dest: Destination,
        message: capnp::message::Reader<T>,
        safety_route_spec: Option<&SafetyRouteSpec>,
    ) -> Result<Option<WaitableReply>, RPCError> {
        log_rpc!(self.get_rpc_request_debug_info(&dest, &message, &safety_route_spec));

        let (op_id, wants_answer) = {
            let operation = message
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_internal!("invalid operation"))
                .map_err(logthru_rpc!(error))?;
            let op_id = operation.get_op_id();
            let wants_answer = self.wants_answer(&operation).map_err(logthru_rpc!())?;

            (op_id, wants_answer)
        };

        let out_node_id; // Envelope Node Id
        let mut out_noderef: Option<NodeRef> = None; // Node to send envelope to
        let hopcount: usize; // Total safety + private route hop count

        // Create envelope data
        let out = {
            let out; // Envelope data

            // To where are we sending the request
            match &dest {
                Destination::Direct(node_ref) | Destination::Relay(node_ref, _) => {
                    // Send to a node without a private route
                    // --------------------------------------

                    // Get the actual destination node id accounting for relays
                    let (node_ref, node_id) = if let Destination::Relay(_, dht_key) = dest {
                        (node_ref.clone(), dht_key)
                    } else {
                        let node_id = node_ref.node_id();
                        (node_ref.clone(), node_id)
                    };

                    // Handle the existence of safety route
                    match safety_route_spec {
                        None => {
                            // If no safety route is being used, and we're not sending to a private
                            // route, we can use a direct envelope instead of routing
                            out = reader_to_vec(message)?;

                            // Message goes directly to the node
                            out_node_id = node_id;
                            out_noderef = Some(node_ref);
                            hopcount = 1;
                        }
                        Some(sr) => {
                            // No private route was specified for the request
                            // but we are using a safety route, so we must create an empty private route
                            let mut pr_builder = ::capnp::message::Builder::new_default();
                            let private_route =
                                self.new_stub_private_route(node_id, &mut pr_builder)?;

                            let message_vec = reader_to_vec(message)?;
                            // first
                            out_node_id = sr
                                .hops
                                .first()
                                .ok_or_else(|| rpc_error_internal("no hop in safety route"))?
                                .dial_info
                                .node_id
                                .key;
                            out = self.wrap_with_route(Some(sr), private_route, message_vec)?;
                            hopcount = 1 + sr.hops.len();
                        }
                    };
                }
                Destination::PrivateRoute(private_route) => {
                    // Send to private route
                    // ---------------------

                    // Encode the private route
                    let mut pr_msg_builder = ::capnp::message::Builder::new_default();
                    let mut pr_builder =
                        pr_msg_builder.init_root::<veilid_capnp::private_route::Builder>();
                    encode_private_route(private_route, &mut pr_builder)?;
                    let pr_reader = pr_builder.into_reader();

                    // Reply with 'route' operation
                    let message_vec = reader_to_vec(message)?;
                    out_node_id = match safety_route_spec {
                        None => {
                            // If no safety route, the first node is the first hop of the private route
                            hopcount = private_route.hop_count as usize;
                            let out_node_id = match &private_route.hops {
                                Some(rh) => rh.dial_info.node_id.key,
                                _ => return Err(rpc_error_internal("private route has no hops")),
                            };
                            out = self.wrap_with_route(None, pr_reader, message_vec)?;
                            out_node_id
                        }
                        Some(sr) => {
                            // If safety route is in use, first node is the first hop of the safety route
                            hopcount = 1 + sr.hops.len() + (private_route.hop_count as usize);
                            let out_node_id = sr
                                .hops
                                .first()
                                .ok_or_else(|| rpc_error_internal("no hop in safety route"))?
                                .dial_info
                                .node_id
                                .key;
                            out = self.wrap_with_route(Some(sr), pr_reader, message_vec)?;
                            out_node_id
                        }
                    }
                }
            }
            out
        };

        // Verify hop count isn't larger than out maximum routed hop count
        if hopcount > self.inner.lock().max_route_hop_count {
            return Err(rpc_error_internal("hop count too long for route"))
                .map_err(logthru_rpc!(warn));
        }
        // calculate actual timeout
        // timeout is number of hops times the timeout per hop
        let timeout = self.inner.lock().timeout * (hopcount as u64);

        // if we need to resolve the first hop, do it
        let node_ref = match out_noderef {
            None => {
                // resolve node
                self.resolve_node(out_node_id)
                    .await
                    .map_err(logthru_rpc!(error))?
            }
            Some(nr) => {
                // got the node in the routing table already
                nr
            }
        };

        // set up op id eventual
        let eventual = if wants_answer {
            Some(self.add_op_id_waiter(op_id))
        } else {
            None
        };

        // send question
        let bytes = out.len() as u64;
        let send_data_kind = match self
            .network_manager()
            .send_envelope(node_ref.clone(), Some(out_node_id), out)
            .await
            .map_err(logthru_rpc!(error))
            .map_err(RPCError::Internal)
        {
            Ok(v) => v,
            Err(e) => {
                // Make sure to clean up op id waiter in case of error
                if eventual.is_some() {
                    self.cancel_op_id_waiter(op_id);
                }
                return Err(e);
            }
        };

        // Successfully sent
        let send_ts = get_timestamp();
        self.routing_table()
            .stats_question_sent(node_ref.clone(), send_ts, bytes, wants_answer);

        // Pass back waitable reply completion
        match eventual {
            None => {
                // if we don't want an answer, don't wait for one
                Ok(None)
            }
            Some(eventual) => Ok(Some(WaitableReply {
                op_id,
                eventual,
                timeout,
                node_ref,
                send_ts,
                send_data_kind,
            })),
        }
    }

    // Issue a reply over the network, possibly using an anonymized route
    // The request must want a response, or this routine fails
    async fn reply<T: capnp::message::ReaderSegments>(
        &self,
        request_rpcreader: RPCMessageReader,
        reply_msg: capnp::message::Reader<T>,
        safety_route_spec: Option<&SafetyRouteSpec>,
    ) -> Result<(), RPCError> {
        log_rpc!(self.get_rpc_reply_debug_info(&request_rpcreader, &reply_msg, &safety_route_spec));

        //
        let out_node_id;
        let mut out_noderef: Option<NodeRef> = None;

        let out = {
            let out;

            let request_operation = request_rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            let reply_vec = reader_to_vec(reply_msg)?;

            // To where should we respond?
            match request_operation
                .get_respond_to()
                .which()
                .map_err(map_error_internal!("invalid request operation"))
                .map_err(logthru_rpc!())?
            {
                veilid_capnp::operation::respond_to::None(_) => {
                    // Do not respond
                    // --------------
                    return Err(rpc_error_internal("no response requested"))
                        .map_err(logthru_rpc!());
                }
                veilid_capnp::operation::respond_to::Sender(_) => {
                    // Respond to envelope source node, possibly through a relay if the request arrived that way
                    // -------------------------------
                    match safety_route_spec {
                        None => {
                            // If no safety route is being used, and we're not replying to a private
                            // route, we can use a direct envelope instead of routing
                            out = reply_vec;

                            // Reply directly to the request's source
                            out_node_id = request_rpcreader.header.envelope.get_sender_id();

                            // This may be a different node's reference than the 'sender' in the case of a relay
                            // But in that case replies to inbound requests are returned through the inbound relay anyway
                            out_noderef = Some(request_rpcreader.header.peer_noderef.clone());
                        }
                        Some(sr) => {
                            // No private route was specified for the return
                            // but we are using a safety route, so we must create an empty private route
                            let mut pr_builder = ::capnp::message::Builder::new_default();
                            let private_route = self
                                .new_stub_private_route(
                                    request_rpcreader.header.envelope.get_sender_id(),
                                    &mut pr_builder,
                                )
                                .map_err(logthru_rpc!())?;

                            out = self.wrap_with_route(Some(sr), private_route, reply_vec)?;
                            // first
                            out_node_id = sr
                                .hops
                                .first()
                                .ok_or_else(|| rpc_error_internal("no hop in safety route"))
                                .map_err(logthru_rpc!())?
                                .dial_info
                                .node_id
                                .key;
                        }
                    };
                }
                veilid_capnp::operation::respond_to::PrivateRoute(pr) => {
                    // Respond to private route
                    // ------------------------

                    // Extract private route for reply
                    let private_route = match pr {
                        Ok(v) => v,
                        Err(_) => {
                            return Err(rpc_error_internal("invalid private route"))
                                .map_err(logthru_rpc!())
                        }
                    };

                    // Reply with 'route' operation
                    out = self.wrap_with_route(safety_route_spec, private_route, reply_vec)?;
                    out_node_id = match safety_route_spec {
                        None => {
                            // If no safety route, the first node is the first hop of the private route
                            if !private_route.has_first_hop() {
                                return Err(rpc_error_internal("private route has no hops"))
                                    .map_err(logthru_rpc!());
                            }
                            let hop = private_route
                                .get_first_hop()
                                .map_err(map_error_internal!("not a valid first hop"))?;
                            decode_public_key(
                                &hop.get_dial_info()
                                    .map_err(map_error_internal!("not a valid dial info"))
                                    .map_err(logthru_rpc!())?
                                    .get_node_id()
                                    .map_err(map_error_internal!("not a valid node id"))
                                    .map_err(logthru_rpc!())?,
                            )
                        }
                        Some(sr) => {
                            // If safety route is in use, first node is the first hop of the safety route
                            sr.hops
                                .first()
                                .ok_or_else(|| rpc_error_internal("no hop in safety route"))
                                .map_err(logthru_rpc!())?
                                .dial_info
                                .node_id
                                .key
                        }
                    }
                }
            }
            out
        };

        // if we need to resolve the first hop, do it
        let node_ref = match out_noderef {
            None => {
                // resolve node
                self.resolve_node(out_node_id).await?
            }
            Some(nr) => {
                // got the node in the routing table already
                nr
            }
        };

        // Send the reply
        let bytes = out.len() as u64;
        self.network_manager()
            .send_envelope(node_ref.clone(), Some(out_node_id), out)
            .await
            .map_err(RPCError::Internal)?;

        // Reply successfully sent
        let send_ts = get_timestamp();

        self.routing_table()
            .stats_answer_sent(node_ref, send_ts, bytes);

        Ok(())
    }

    fn wants_answer(&self, operation: &veilid_capnp::operation::Reader) -> Result<bool, RPCError> {
        match operation.get_respond_to().which() {
            Ok(veilid_capnp::operation::respond_to::None(_)) => Ok(false),
            Ok(veilid_capnp::operation::respond_to::Sender(_)) => Ok(true),
            Ok(veilid_capnp::operation::respond_to::PrivateRoute(_)) => Ok(true),
            _ => Err(rpc_error_internal("Unknown respond_to")),
        }
    }

    fn get_respond_to_sender_node_info(
        &self,
        operation: &veilid_capnp::operation::Reader,
    ) -> Result<Option<NodeInfo>, RPCError> {
        if let veilid_capnp::operation::respond_to::Sender(Ok(sender_ni_reader)) = operation
            .get_respond_to()
            .which()
            .map_err(map_error_capnp_notinschema!())?
        {
            // Sender DialInfo was specified, update our routing table with it
            Ok(Some(decode_node_info(&sender_ni_reader, true)?))
        } else {
            Ok(None)
        }
    }

    //////////////////////////////////////////////////////////////////////

    async fn generate_sender_info(&self, peer_noderef: NodeRef) -> SenderInfo {
        let socket_address = peer_noderef
            .last_connection()
            .await
            .map(|c| c.remote.socket_address);
        SenderInfo { socket_address }
    }

    async fn process_info_q(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        let peer_noderef = rpcreader.header.peer_noderef.clone();
        let sender_info = self.generate_sender_info(peer_noderef).await;

        let reply_msg = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // Don't bother unless we are going to answer
            if !self.wants_answer(&operation)? {
                return Ok(());
            }

            // get InfoQ reader
            let iq_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::InfoQ(Ok(x))) => x,
                _ => panic!("invalid operation type in process_info_q"),
            };

            // Parse out fields
            let node_status = decode_node_status(
                &iq_reader
                    .get_node_status()
                    .map_err(map_error_internal!("no valid node status"))?,
            )?;

            // update node status for the requesting node to our routing table
            if let Some(sender_nr) = rpcreader.opt_sender_nr.clone() {
                // Update latest node status in routing table for the infoq sender
                sender_nr.operate(|e| {
                    e.update_node_status(node_status);
                });
            }

            // Send info answer
            let mut reply_msg = ::capnp::message::Builder::new_default();
            let mut answer = reply_msg.init_root::<veilid_capnp::operation::Builder>();
            answer.set_op_id(operation.get_op_id());
            let mut respond_to = answer.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = answer.reborrow().init_detail();
            let mut info_a = detail.init_info_a();

            // Add node status
            let node_status = self.network_manager().generate_node_status();
            let mut nsb = info_a.reborrow().init_node_status();
            encode_node_status(&node_status, &mut nsb)?;

            // Add sender info
            let mut sib = info_a.reborrow().init_sender_info();
            encode_sender_info(&sender_info, &mut sib)?;

            reply_msg.into_reader()
        };

        self.reply(rpcreader, reply_msg, None).await
    }

    async fn process_validate_dial_info(
        &self,
        rpcreader: RPCMessageReader,
    ) -> Result<(), RPCError> {
        //
        let (alternate_port, redirect, dial_info, rcpt_data) = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(RPCError::InvalidFormat);
            }

            // get validateDialInfo reader
            let vdi_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::ValidateDialInfo(Ok(x))) => x,
                _ => panic!("invalid operation type in process_validate_dial_info"),
            };

            // Parse out fields
            let alternate_port = vdi_reader.get_alternate_port();
            let redirect = vdi_reader.get_redirect();
            let dial_info = decode_dial_info(
                &vdi_reader
                    .get_dial_info()
                    .map_err(map_error_internal!("no valid dial info"))?,
            )?;
            let rcpt_data = vdi_reader
                .get_receipt()
                .map_err(map_error_internal!("no valid receipt"))?;

            (alternate_port, redirect, dial_info, rcpt_data)
        };

        // Redirect this request if we are asked to
        if redirect {
            let routing_table = self.routing_table();
            let filter = dial_info.make_filter(true);
            let peers = routing_table.find_fast_public_nodes_filtered(&filter);
            if peers.is_empty() {
                return Err(rpc_error_internal(format!(
                    "no peers matching filter '{:?}'",
                    filter
                )));
            }
            for peer in peers {
                // See if this peer will validate dial info
                let will_validate_dial_info = peer.operate(|e: &mut BucketEntry| {
                    if let Some(ni) = &e.peer_stats().status {
                        ni.will_validate_dial_info
                    } else {
                        true
                    }
                });
                if !will_validate_dial_info {
                    continue;
                }
                // Make a copy of the request, without the redirect flag
                let vdi_msg_reader = {
                    let mut vdi_msg = ::capnp::message::Builder::new_default();
                    let mut question = vdi_msg.init_root::<veilid_capnp::operation::Builder>();
                    question.set_op_id(self.get_next_op_id());
                    let mut respond_to = question.reborrow().init_respond_to();
                    respond_to.set_none(());
                    let detail = question.reborrow().init_detail();
                    let mut vdi_builder = detail.init_validate_dial_info();
                    vdi_builder.set_alternate_port(alternate_port);
                    vdi_builder.set_redirect(false);
                    let mut di_builder = vdi_builder.reborrow().init_dial_info();
                    encode_dial_info(&dial_info, &mut di_builder)?;
                    let r_builder = vdi_builder.reborrow().init_receipt(
                        rcpt_data
                            .len()
                            .try_into()
                            .map_err(map_error_internal!("receipt too large"))?,
                    );
                    r_builder.copy_from_slice(rcpt_data);

                    vdi_msg.into_reader()
                };

                // Send the validate_dial_info request until we succeed
                self.request(Destination::Direct(peer.clone()), vdi_msg_reader, None)
                    .await?;
            }
            return Ok(());
        };

        // Otherwise send a return receipt directly
        // Possibly from an alternate port
        let network_manager = self.network_manager();
        network_manager
            .send_direct_receipt(dial_info.clone(), rcpt_data, alternate_port)
            .await
            .map_err(map_error_string!())
            .map_err(
                logthru_net!(error "failed to send direct receipt to dial info: {}, alternate_port={}", dial_info, alternate_port),
            )?;

        Ok(())
    }

    async fn process_find_node_q(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        //
        let reply_msg = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // find_node must always want an answer
            if !self.wants_answer(&operation)? {
                return Err(RPCError::InvalidFormat).map_err(logthru_rpc!());
            }

            // get findNodeQ reader
            let fnq_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::FindNodeQ(Ok(x))) => x,
                _ => panic!("invalid operation type in process_find_node_q"),
            };

            // get the node id we want to look up
            let target_node_id = decode_public_key(
                &fnq_reader
                    .get_node_id()
                    .map_err(map_error_capnp_error!())
                    .map_err(logthru_rpc!())?,
            );

            // add node information for the requesting node to our routing table
            let routing_table = self.routing_table();

            // find N nodes closest to the target node in our routing table
            let own_peer_info = routing_table.get_own_peer_info();
            let closest_nodes = routing_table.find_closest_nodes(
                target_node_id,
                // filter
                None,
                // transform
                |e| RoutingTable::transform_to_peer_info(e, &own_peer_info),
            );
            log_rpc!(">>>> Returning {} closest peers", closest_nodes.len());

            // Send find_node answer
            let mut reply_msg = ::capnp::message::Builder::new_default();
            let mut answer = reply_msg.init_root::<veilid_capnp::operation::Builder>();
            answer.set_op_id(operation.get_op_id());
            let mut respond_to = answer.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = answer.reborrow().init_detail();
            let info_a = detail.init_find_node_a();
            let mut peers_builder = info_a.init_peers(
                closest_nodes
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("invalid closest nodes list length"))?,
            );
            for (i, closest_node) in closest_nodes.iter().enumerate() {
                let mut pi_builder = peers_builder.reborrow().get(i as u32);
                encode_peer_info(closest_node, &mut pi_builder)?;
            }
            reply_msg.into_reader()
        };

        self.reply(rpcreader, reply_msg, None).await
    }

    async fn process_route(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        // xxx do not process latency for routed messages
        Err(rpc_error_unimplemented("process_route"))
    }

    async fn process_get_value_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_get_value_q"))
    }

    async fn process_set_value_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_set_value_q"))
    }

    async fn process_watch_value_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_watch_value_q"))
    }

    async fn process_value_changed(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_value_changed"))
    }

    async fn process_supply_block_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_supply_block_q"))
    }

    async fn process_find_block_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_find_block_q"))
    }

    async fn process_signal(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        let signal_info = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(RPCError::InvalidFormat);
            }

            // get signal reader
            let sig_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::Signal(Ok(x))) => x,
                _ => panic!("invalid operation type in process_signal"),
            };

            // Get signal info
            decode_signal_info(&sig_reader)?
        };

        // Handle it
        let network_manager = self.network_manager();
        network_manager
            .handle_signal(signal_info)
            .await
            .map_err(map_error_string!())
    }

    async fn process_return_receipt(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        let rcpt_data = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            // This should never want an answer
            if self.wants_answer(&operation)? {
                return Err(RPCError::InvalidFormat);
            }

            // get returnReceipt reader
            let rr_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::ReturnReceipt(Ok(x))) => x,
                _ => panic!("invalid operation type in process_return_receipt"),
            };

            // Get receipt data
            let rcpt_data = rr_reader
                .get_receipt()
                .map_err(map_error_internal!("no valid receipt"))?;

            rcpt_data.to_vec()
        };

        // Handle it
        let network_manager = self.network_manager();
        network_manager
            .handle_in_band_receipt(rcpt_data, rpcreader.header.peer_noderef)
            .await
            .map_err(map_error_string!())
    }

    async fn process_start_tunnel_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_start_tunnel_q"))
    }

    async fn process_complete_tunnel_q(
        &self,
        _rpcreader: RPCMessageReader,
    ) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_complete_tunnel_q"))
    }

    async fn process_cancel_tunnel_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_cancel_tunnel_q"))
    }

    async fn process_answer(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        // pass answer to the appropriate rpc waiter
        let op_id = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;
            operation.get_op_id()
        };

        Ok(self.complete_op_id_waiter(op_id, rpcreader).await?)
    }

    //////////////////////////////////////////////////////////////////////
    async fn process_rpc_message_version_0(&self, msg: RPCMessage) -> Result<(), RPCError> {
        let reader = capnp::message::Reader::new(msg.data, Default::default());
        let mut opt_sender_nr: Option<NodeRef> = None;
        let which = {
            let operation = reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;

            let (which, is_q) = match operation
                .get_detail()
                .which()
                .map_err(map_error_capnp_notinschema!())?
            {
                veilid_capnp::operation::detail::InfoQ(_) => (0u32, true),
                veilid_capnp::operation::detail::InfoA(_) => (1u32, false),
                veilid_capnp::operation::detail::ValidateDialInfo(_) => (2u32, true),
                veilid_capnp::operation::detail::FindNodeQ(_) => (3u32, true),
                veilid_capnp::operation::detail::FindNodeA(_) => (4u32, false),
                veilid_capnp::operation::detail::Route(_) => (5u32, true),
                veilid_capnp::operation::detail::GetValueQ(_) => (6u32, true),
                veilid_capnp::operation::detail::GetValueA(_) => (7u32, false),
                veilid_capnp::operation::detail::SetValueQ(_) => (8u32, true),
                veilid_capnp::operation::detail::SetValueA(_) => (9u32, false),
                veilid_capnp::operation::detail::WatchValueQ(_) => (10u32, true),
                veilid_capnp::operation::detail::WatchValueA(_) => (11u32, false),
                veilid_capnp::operation::detail::ValueChanged(_) => (12u32, true),
                veilid_capnp::operation::detail::SupplyBlockQ(_) => (13u32, true),
                veilid_capnp::operation::detail::SupplyBlockA(_) => (14u32, false),
                veilid_capnp::operation::detail::FindBlockQ(_) => (15u32, true),
                veilid_capnp::operation::detail::FindBlockA(_) => (16u32, false),
                veilid_capnp::operation::detail::Signal(_) => (17u32, true),
                veilid_capnp::operation::detail::ReturnReceipt(_) => (18u32, true),
                veilid_capnp::operation::detail::StartTunnelQ(_) => (19u32, true),
                veilid_capnp::operation::detail::StartTunnelA(_) => (20u32, false),
                veilid_capnp::operation::detail::CompleteTunnelQ(_) => (21u32, true),
                veilid_capnp::operation::detail::CompleteTunnelA(_) => (22u32, false),
                veilid_capnp::operation::detail::CancelTunnelQ(_) => (23u32, true),
                veilid_capnp::operation::detail::CancelTunnelA(_) => (24u32, false),
            };

            // Accounting for questions we receive
            if is_q {
                // See if we have some Sender NodeInfo to incorporate
                opt_sender_nr =
                    if let Some(sender_ni) = self.get_respond_to_sender_node_info(&operation)? {
                        // Sender NodeInfo was specified, update our routing table with it
                        if !self.filter_peer_scope(&sender_ni) {
                            return Err(RPCError::InvalidFormat);
                        }
                        let nr = self
                            .routing_table()
                            .register_node_with_node_info(
                                msg.header.envelope.get_sender_id(),
                                sender_ni,
                            )
                            .map_err(RPCError::Internal)?;
                        Some(nr)
                    } else {
                        // look up sender node, in case it's different than our peer due to relaying
                        self.routing_table()
                            .lookup_node_ref(msg.header.envelope.get_sender_id())
                    };

                if let Some(sender_nr) = opt_sender_nr.clone() {
                    self.routing_table().stats_question_rcvd(
                        sender_nr,
                        msg.header.timestamp,
                        msg.header.body_len,
                    );
                }
            };

            which
        };

        let rpcreader = RPCMessageReader {
            header: msg.header,
            reader,
            opt_sender_nr,
        };

        match which {
            0 => self.process_info_q(rpcreader).await, // InfoQ
            1 => self.process_answer(rpcreader).await, // InfoA
            2 => self.process_validate_dial_info(rpcreader).await, // ValidateDialInfo
            3 => self.process_find_node_q(rpcreader).await, // FindNodeQ
            4 => self.process_answer(rpcreader).await, // FindNodeA
            5 => self.process_route(rpcreader).await,  // Route
            6 => self.process_get_value_q(rpcreader).await, // GetValueQ
            7 => self.process_answer(rpcreader).await, // GetValueA
            8 => self.process_set_value_q(rpcreader).await, // SetValueQ
            9 => self.process_answer(rpcreader).await, // SetValueA
            10 => self.process_watch_value_q(rpcreader).await, // WatchValueQ
            11 => self.process_answer(rpcreader).await, // WatchValueA
            12 => self.process_value_changed(rpcreader).await, // ValueChanged
            13 => self.process_supply_block_q(rpcreader).await, // SupplyBlockQ
            14 => self.process_answer(rpcreader).await, // SupplyBlockA
            15 => self.process_find_block_q(rpcreader).await, // FindBlockQ
            16 => self.process_answer(rpcreader).await, // FindBlockA
            17 => self.process_signal(rpcreader).await, // SignalQ
            18 => self.process_return_receipt(rpcreader).await, // ReturnReceipt
            19 => self.process_start_tunnel_q(rpcreader).await, // StartTunnelQ
            20 => self.process_answer(rpcreader).await, // StartTunnelA
            21 => self.process_complete_tunnel_q(rpcreader).await, // CompleteTunnelQ
            22 => self.process_answer(rpcreader).await, // CompleteTunnelA
            23 => self.process_cancel_tunnel_q(rpcreader).await, // CancelTunnelQ
            24 => self.process_answer(rpcreader).await, // CancelTunnelA
            _ => panic!("must update rpc table"),
        }
    }

    async fn process_rpc_message(&self, msg: RPCMessage) -> Result<(), RPCError> {
        if msg.header.envelope.get_version() == 0 {
            self.process_rpc_message_version_0(msg).await
        } else {
            Err(RPCError::Internal(format!(
                "unsupported envelope version: {}, newest supported is version 0",
                msg.header.envelope.get_version()
            )))
        }
    }

    async fn rpc_worker(self, receiver: flume::Receiver<RPCMessage>) {
        while let Ok(msg) = receiver.recv_async().await {
            let _ = self
                .process_rpc_message(msg)
                .await
                .map_err(logthru_rpc!("couldn't process rpc message"));
        }
    }

    pub async fn startup(&self) -> Result<(), String> {
        trace!("startup rpc processor");
        let mut inner = self.inner.lock();
        // make local copy of node id for easy access
        let c = self.config.get();
        inner.node_id = c.network.node_id;
        inner.node_id_secret = c.network.node_id_secret;

        // set up channel
        let mut concurrency = c.network.rpc.concurrency;
        let mut queue_size = c.network.rpc.queue_size;
        let mut timeout = ms_to_us(c.network.rpc.timeout_ms);
        let mut max_route_hop_count = c.network.rpc.max_route_hop_count as usize;
        if concurrency == 0 {
            concurrency = get_concurrency() / 2;
            if concurrency == 0 {
                concurrency = 1;
            }
        }
        if queue_size == 0 {
            queue_size = 1024;
        }
        if timeout == 0 {
            timeout = 10000000;
        }
        if max_route_hop_count == 0 {
            max_route_hop_count = 7usize;
        }
        inner.timeout = timeout;
        inner.max_route_hop_count = max_route_hop_count;
        let channel = flume::bounded(queue_size as usize);
        inner.send_channel = Some(channel.0.clone());

        // spin up N workers
        trace!("Spinning up {} RPC workers", concurrency);
        for _ in 0..concurrency {
            let this = self.clone();
            let receiver = channel.1.clone();
            let jh = spawn(Self::rpc_worker(this, receiver));
            inner.worker_join_handles.push(jh);
        }

        Ok(())
    }

    pub async fn shutdown(&self) {
        *self.inner.lock() = Self::new_inner(self.network_manager());
    }

    pub fn enqueue_message(
        &self,
        envelope: envelope::Envelope,
        body: Vec<u8>,
        peer_noderef: NodeRef,
    ) -> Result<(), String> {
        let msg = RPCMessage {
            header: RPCMessageHeader {
                timestamp: get_timestamp(),
                envelope,
                body_len: body.len() as u64,
                peer_noderef,
            },
            data: RPCMessageData { contents: body },
        };
        let send_channel = {
            let inner = self.inner.lock();
            inner.send_channel.as_ref().unwrap().clone()
        };
        send_channel
            .try_send(msg)
            .map_err(|e| format!("failed to enqueue received RPC message: {:?}", e))?;
        Ok(())
    }

    // Gets a 'RespondTo::Sender' that contains either our dial info,
    // or None if the peer has seen our dial info before or our node info is not yet valid
    // because of an unknown network class
    pub fn make_respond_to_sender(&self, peer: NodeRef) -> RespondTo {
        let our_node_info = self.routing_table().get_own_peer_info().node_info;
        if peer.has_seen_our_node_info()
            || matches!(our_node_info.network_class, NetworkClass::Invalid)
        {
            RespondTo::Sender(None)
        } else {
            RespondTo::Sender(Some(our_node_info))
        }
    }

    // Send InfoQ RPC request, receive InfoA answer
    // Can be sent via relays, but not via routes
    pub async fn rpc_call_info(self, peer: NodeRef) -> Result<InfoAnswer, RPCError> {
        let info_q_msg = {
            let mut info_q_msg = ::capnp::message::Builder::new_default();
            let mut question = info_q_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            self.make_respond_to_sender(peer.clone())
                .encode(&mut respond_to)?;
            let detail = question.reborrow().init_detail();
            let mut iqb = detail.init_info_q();
            let mut node_status_builder = iqb.reborrow().init_node_status();
            let node_status = self.network_manager().generate_node_status();
            encode_node_status(&node_status, &mut node_status_builder)?;

            info_q_msg.into_reader()
        };

        // Send the info request
        let waitable_reply = self
            .request(Destination::Direct(peer.clone()), info_q_msg, None)
            .await?
            .unwrap();

        // Note what kind of ping this was and to what peer scope
        let send_data_kind = waitable_reply.send_data_kind;

        // Wait for reply
        let (rpcreader, latency) = self.wait_for_reply(waitable_reply).await?;

        let (sender_info, node_status) = {
            let response_operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())
                .map_err(logthru_rpc!())?;
            let info_a = match response_operation
                .get_detail()
                .which()
                .map_err(map_error_capnp_notinschema!())
                .map_err(logthru_rpc!())?
            {
                veilid_capnp::operation::detail::InfoA(a) => {
                    a.map_err(map_error_internal!("Invalid InfoA"))?
                }
                _ => return Err(rpc_error_internal("Incorrect RPC answer for question")),
            };

            // Decode node info
            if !info_a.has_node_status() {
                return Err(rpc_error_internal("Missing node status"));
            }
            let nsr = info_a
                .get_node_status()
                .map_err(map_error_internal!("Broken node status"))?;
            let node_status = decode_node_status(&nsr)?;

            // Decode sender info
            let sender_info = if info_a.has_sender_info() {
                let sir = info_a
                    .get_sender_info()
                    .map_err(map_error_internal!("Broken sender info"))?;
                decode_sender_info(&sir)?
            } else {
                SenderInfo::default()
            };

            // Update latest node status in routing table
            peer.operate(|e| {
                e.update_node_status(node_status.clone());
            });

            (sender_info, node_status)
        };

        // Report sender_info IP addresses to network manager
        if let Some(socket_address) = sender_info.socket_address {
            match send_data_kind {
                SendDataKind::LocalDirect => {
                    self.network_manager()
                        .report_local_socket_address(socket_address, peer)
                        .await;
                }
                SendDataKind::GlobalDirect => {
                    self.network_manager()
                        .report_global_socket_address(socket_address, peer)
                        .await;
                }
                SendDataKind::GlobalIndirect => {
                    // Do nothing in this case, as the socket address returned here would be for any node other than ours
                }
            }
        }

        // Return the answer for anyone who may care
        let out = InfoAnswer {
            latency,
            node_status,
            sender_info,
        };

        Ok(out)
    }

    // Can only be sent directly, not via relays or routes
    pub async fn rpc_call_validate_dial_info(
        &self,
        peer: NodeRef,
        dial_info: DialInfo,
        redirect: bool,
        alternate_port: bool,
    ) -> Result<bool, RPCError> {
        let network_manager = self.network_manager();
        let receipt_time = ms_to_us(
            self.config
                .get()
                .network
                .dht
                .validate_dial_info_receipt_time_ms,
        );
        //
        let (vdi_msg, eventual_value) = {
            let mut vdi_msg = ::capnp::message::Builder::new_default();
            let mut question = vdi_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let mut vdi_builder = detail.init_validate_dial_info();

            // Generate receipt and waitable eventual so we can see if we get the receipt back
            let (rcpt_data, eventual_value) = network_manager
                .generate_single_shot_receipt(receipt_time, [])
                .map_err(map_error_string!())?;

            vdi_builder.set_redirect(redirect);
            vdi_builder.set_alternate_port(alternate_port);
            let mut di_builder = vdi_builder.reborrow().init_dial_info();
            encode_dial_info(&dial_info, &mut di_builder)?;
            let r_builder = vdi_builder.reborrow().init_receipt(
                rcpt_data
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("receipt too large"))?,
            );
            r_builder.copy_from_slice(rcpt_data.as_slice());
            (vdi_msg.into_reader(), eventual_value)
        };

        // Send the validate_dial_info request
        // This can only be sent directly, as relays can not validate dial info
        self.request(Destination::Direct(peer), vdi_msg, None)
            .await?;

        // Wait for receipt
        match eventual_value.await {
            ReceiptEvent::Returned(_) => Ok(true),
            ReceiptEvent::Expired => Ok(false),
            ReceiptEvent::Cancelled => {
                Err(rpc_error_internal("receipt was dropped before expiration"))
            }
        }
    }

    // Send FindNodeQ RPC request, receive FindNodeA answer
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_find_node(
        self,
        dest: Destination,
        key: key::DHTKey,
        safety_route: Option<&SafetyRouteSpec>,
        respond_to: RespondTo,
    ) -> Result<FindNodeAnswer, RPCError> {
        let find_node_q_msg = {
            let mut find_node_q_msg = ::capnp::message::Builder::new_default();
            let mut question = find_node_q_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to_builder = question.reborrow().init_respond_to();
            respond_to.encode(&mut respond_to_builder)?;
            let detail = question.reborrow().init_detail();
            let mut fnq = detail.init_find_node_q();
            let mut node_id_builder = fnq.reborrow().init_node_id();
            encode_public_key(&key, &mut node_id_builder)?;

            find_node_q_msg.into_reader()
        };

        // Send the find_node request
        let waitable_reply = self
            .request(dest, find_node_q_msg, safety_route)
            .await?
            .unwrap();

        // Wait for reply
        let (rpcreader, latency) = self.wait_for_reply(waitable_reply).await?;

        let response_operation = rpcreader
            .reader
            .get_root::<veilid_capnp::operation::Reader>()
            .map_err(map_error_capnp_error!())
            .map_err(logthru_rpc!())?;
        let find_node_a = match response_operation
            .get_detail()
            .which()
            .map_err(map_error_capnp_notinschema!())
            .map_err(logthru_rpc!())?
        {
            veilid_capnp::operation::detail::FindNodeA(a) => {
                a.map_err(map_error_internal!("Invalid FindNodeA"))?
            }
            _ => return Err(rpc_error_internal("Incorrect RPC answer for question")),
        };

        let peers_reader = find_node_a
            .get_peers()
            .map_err(map_error_internal!("Missing peers"))?;
        let mut peers = Vec::<PeerInfo>::with_capacity(
            peers_reader
                .len()
                .try_into()
                .map_err(map_error_internal!("too many peers"))?,
        );
        for p in peers_reader.iter() {
            let peer_info = decode_peer_info(&p, true)?;

            if !self.filter_peer_scope(&peer_info.node_info) {
                return Err(RPCError::InvalidFormat);
            }

            peers.push(peer_info);
        }

        let out = FindNodeAnswer { latency, peers };

        Ok(out)
    }

    // Sends a unidirectional signal to a node
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_signal(
        &self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
        signal_info: SignalInfo,
    ) -> Result<(), RPCError> {
        let sig_msg = {
            let mut sig_msg = ::capnp::message::Builder::new_default();
            let mut question = sig_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let mut sig_builder = detail.init_signal();
            encode_signal_info(&signal_info, &mut sig_builder)?;

            sig_msg.into_reader()
        };

        // Send the signal request
        self.request(dest, sig_msg, safety_route).await?;

        Ok(())
    }

    // Sends a unidirectional in-band return receipt
    // Can be sent via all methods including relays and routes
    pub async fn rpc_call_return_receipt<B: AsRef<[u8]>>(
        &self,
        dest: Destination,
        safety_route: Option<&SafetyRouteSpec>,
        rcpt_data: B,
    ) -> Result<(), RPCError> {
        // Validate receipt before we send it, otherwise this may be arbitrary data!
        let _ = Receipt::from_signed_data(rcpt_data.as_ref())
            .map_err(|_| "failed to validate direct receipt".to_owned())
            .map_err(map_error_string!())?;

        let rr_msg = {
            let mut rr_msg = ::capnp::message::Builder::new_default();
            let mut question = rr_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = question.reborrow().init_detail();
            let rr_builder = detail.init_return_receipt();

            let r_builder = rr_builder.init_receipt(rcpt_data.as_ref().len().try_into().map_err(
                map_error_protocol!("invalid receipt length in return receipt"),
            )?);
            r_builder.copy_from_slice(rcpt_data.as_ref());

            rr_msg.into_reader()
        };

        // Send the return receipt request
        self.request(dest, rr_msg, safety_route).await?;

        Ok(())
    }

    // xxx do not process latency for routed messages
}
