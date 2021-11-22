mod coders;

use crate::dht::*;
use crate::intf::utils::channel::*;
use crate::intf::*;
use crate::xx::*;
use crate::*;
use capnp::message::ReaderSegments;
use coders::*;
use core::convert::{TryFrom, TryInto};
use core::fmt;
use lease_manager::*;
use network_manager::*;
use receipt_manager::*;
use routing_table::*;

/////////////////////////////////////////////////////////////////////

type OperationId = u64;

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub enum RPCError {
    Timeout,
    InvalidFormat,
    Unimplemented(String),
    Protocol(String),
    Internal(String),
}
pub fn rpc_error_internal<T: AsRef<str>>(x: T) -> RPCError {
    error!("RPCError Internal: {}", x.as_ref());
    RPCError::Internal(x.as_ref().to_owned())
}
pub fn rpc_error_capnp_error(e: capnp::Error) -> RPCError {
    error!("RPCError Protocol: {}", &e.description);
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
            RPCError::InvalidFormat => write!(f, "[RPCError: InvalidFormat]"),
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

#[derive(Clone)]
pub enum Destination {
    Direct(NodeRef),
    PrivateRoute(PrivateRoute),
}

#[derive(Clone)]
pub enum RespondTo {
    None,
    Sender,
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
            Self::Sender => {
                builder.set_sender(());
            }
            Self::PrivateRoute(pr) => {
                let mut pr_builder = builder.reborrow().init_private_route();
                encode_private_route(&pr, &mut pr_builder)?;
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
    fn get_segment<'a>(&'a self, idx: u32) -> Option<&'a [u8]> {
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
}

fn builder_to_vec<'a, T>(builder: capnp::message::Builder<T>) -> Result<Vec<u8>, RPCError>
where
    T: capnp::message::Allocator + 'a,
{
    let wordvec = builder
        .into_reader()
        .canonicalize()
        .map_err(map_error_capnp_error!())?;
    Ok(capnp::Word::words_to_bytes(wordvec.as_slice()).to_vec())
}
fn reader_to_vec<'a, T>(reader: capnp::message::Reader<T>) -> Result<Vec<u8>, RPCError>
where
    T: capnp::message::ReaderSegments + 'a,
{
    let wordvec = reader.canonicalize().map_err(map_error_capnp_error!())?;
    Ok(capnp::Word::words_to_bytes(wordvec.as_slice()).to_vec())
}

#[derive(Debug)]
struct WaitableReply {
    op_id: OperationId,
    eventual: EventualValue<RPCMessageReader>,
    timeout: u64,
    node_ref: NodeRef,
    send_ts: u64,
    is_ping: bool,
}

/////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct InfoAnswer {
    pub latency: u64,
    pub node_info: NodeInfo,
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
    channel: Option<(Sender<RPCMessage>, Receiver<RPCMessage>)>,
    timeout: u64,
    max_route_hop_count: usize,
    waiting_rpc_table: BTreeMap<OperationId, EventualValue<RPCMessageReader>>,
    worker_join_handles: Vec<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct RPCProcessor {
    crypto: Crypto,
    config: VeilidConfig,
    inner: Arc<Mutex<RPCProcessorInner>>,
}

impl RPCProcessor {
    fn new_inner(network_manager: NetworkManager) -> RPCProcessorInner {
        RPCProcessorInner {
            network_manager: network_manager.clone(),
            routing_table: network_manager.routing_table(),
            node_id: key::DHTKey::default(),
            node_id_secret: key::DHTKeySecret::default(),
            channel: None,
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

    //////////////////////////////////////////////////////////////////////

    // Search the DHT for a single node closest to a key unless we have that node in our routing table already, and return the node reference
    pub async fn search_dht_single_key(
        &self,
        node_id: key::DHTKey,
        _count: u32,
        _fanout: u32,
        _timeout: Option<u64>,
    ) -> Result<NodeRef, RPCError> {
        let routing_table = self.routing_table();

        // First see if we have the node in our routing table already
        if let Some(nr) = routing_table.lookup_node_ref(node_id) {
            // ensure we have dial_info for the entry already,
            // if not, we should do the find_node anyway
            if nr.operate(|e| e.best_dial_info().is_some()) {
                return Ok(nr);
            }
        }

        // xxx find node but stop if we find the exact node we want
        // xxx return whatever node is closest after the timeout
        Err(rpc_error_unimplemented("search_dht_single_key"))
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
        Err(rpc_error_unimplemented("search_dht_multi_key"))
    }

    // Search the DHT for a specific node corresponding to a key unless we have that node in our routing table already, and return the node reference
    pub async fn resolve_node(&self, node_id: key::DHTKey) -> Result<NodeRef, RPCError> {
        let (count, fanout, timeout) = {
            let c = self.config.get();
            (
                c.network.dht.resolve_node_count,
                c.network.dht.resolve_node_fanout,
                c.network.dht.resolve_node_timeout,
            )
        };

        let nr = self
            .search_dht_single_key(node_id, count, fanout, timeout)
            .await?;

        if nr.node_id() != node_id {
            // found a close node, but not exact within our configured resolve_node timeout
            return Err(RPCError::Timeout);
        }

        Ok(nr)
    }

    //////////////////////////////////////////////////////////////////////
    fn new_stub_private_route<'a, T>(
        &self,
        dest_node_id: key::DHTKey,
        builder: &'a mut ::capnp::message::Builder<T>,
    ) -> Result<veilid_capnp::private_route::Reader<'a>, RPCError>
    where
        T: capnp::message::Allocator + 'a,
    {
        let mut pr = builder.init_root::<veilid_capnp::private_route::Builder>();

        let mut pr_pk = pr.reborrow().init_public_key();
        encode_public_key(&dest_node_id, &mut pr_pk)?;
        pr.set_hop_count(0u8);
        // leave firstHop as null
        Ok(pr.into_reader())
    }

    fn encode_safety_route<'a>(
        &self,
        safety_route: &SafetyRouteSpec,
        private_route: veilid_capnp::private_route::Reader<'a>,
        builder: &'a mut veilid_capnp::safety_route::Builder<'a>,
    ) -> Result<(), RPCError> {
        // Ensure the total hop count isn't too long for our config
        let pr_hopcount = private_route.get_hop_count() as usize;
        let sr_hopcount = safety_route.hops.len();
        let hopcount = 1 + sr_hopcount + pr_hopcount;
        if hopcount > self.inner.lock().max_route_hop_count {
            return Err(rpc_error_internal("hop count too long for route"));
        }

        // Build the safety route
        let mut sr_pk = builder.reborrow().init_public_key();
        encode_public_key(&safety_route.public_key, &mut sr_pk)?;

        builder.set_hop_count(
            u8::try_from(sr_hopcount)
                .map_err(map_error_internal!("hop count too large for safety route"))?,
        );

        // Build all the hops in the safety route
        let mut hops_builder = builder.reborrow().init_hops();
        if sr_hopcount == 0 {
            hops_builder
                .set_private(private_route)
                .map_err(map_error_internal!(
                    "invalid private route while encoding safety route"
                ))?;
        } else {
            // start last blob-to-encrypt data off as private route
            let mut blob_data = {
                let mut pr_message = ::capnp::message::Builder::new_default();
                pr_message
                    .set_root_canonical(private_route)
                    .map_err(map_error_internal!(
                        "invalid private route while encoding safety route"
                    ))?;
                let mut blob_data = builder_to_vec(pr_message)?;
                // append the private route tag so we know how to decode it later
                blob_data.push(1u8);
                blob_data
            };

            // Encode each hop from inside to outside
            // skips the outermost hop since that's entering the
            // safety route and does not include the dialInfo
            // (outer hop is a RouteHopData, not a RouteHop).
            // Each loop mutates 'nonce', and 'blob_data'
            let mut nonce = Crypto::get_random_nonce();
            for h in (1..sr_hopcount).rev() {
                // Get blob to encrypt for next hop
                blob_data = {
                    // RouteHop
                    let mut rh_message = ::capnp::message::Builder::new_default();
                    let mut rh_builder = rh_message.init_root::<veilid_capnp::route_hop::Builder>();
                    let mut di_builder = rh_builder.reborrow().init_dial_info();
                    encode_node_dial_info_single(&safety_route.hops[h].dial_info, &mut di_builder)?;
                    // RouteHopData
                    let mut rhd_builder = rh_builder.init_next_hop();
                    // Add the nonce
                    let mut rhd_nonce = rhd_builder.reborrow().init_nonce();
                    encode_nonce(&nonce, &mut rhd_nonce);
                    // Encrypt the previous blob ENC(nonce, DH(PKhop,SKsr))
                    let dh_secret = self
                        .crypto
                        .cached_dh(
                            &safety_route.hops[h].dial_info.node_id.key,
                            &safety_route.secret_key,
                        )
                        .map_err(map_error_internal!("dh failed"))?;
                    let enc_msg_data =
                        Crypto::encrypt(blob_data.as_slice(), &nonce, &dh_secret, None)
                            .map_err(map_error_internal!("encryption failed"))?;

                    rhd_builder.set_blob(enc_msg_data.as_slice());
                    let mut blob_data = builder_to_vec(rh_message)?;

                    // append the route hop tag so we know how to decode it later
                    blob_data.push(0u8);
                    blob_data
                };
                // Make another nonce for the next hop
                nonce = Crypto::get_random_nonce();
            }

            // Encode first RouteHopData
            let mut first_rhd_builder = hops_builder.init_data();
            let mut first_rhd_nonce = first_rhd_builder.reborrow().init_nonce();
            encode_nonce(&nonce, &mut first_rhd_nonce);
            let dh_secret = self
                .crypto
                .cached_dh(
                    &safety_route.hops[0].dial_info.node_id.key,
                    &safety_route.secret_key,
                )
                .map_err(map_error_internal!("dh failed"))?;
            let enc_msg_data = Crypto::encrypt(blob_data.as_slice(), &nonce, &dh_secret, None)
                .map_err(map_error_internal!("encryption failed"))?;

            first_rhd_builder.set_blob(enc_msg_data.as_slice());
        }

        Ok(())
    }

    // Wrap an operation inside a route
    fn wrap_with_route<'a>(
        &self,
        safety_route: Option<&SafetyRouteSpec>,
        private_route: veilid_capnp::private_route::Reader<'a>,
        message_data: Vec<u8>,
    ) -> Result<Vec<u8>, RPCError> {
        // Get stuff before we lock inner
        let op_id = self.get_next_op_id();
        // Encrypt routed operation
        let nonce = Crypto::get_random_nonce();
        let pr_pk_reader = private_route
            .get_public_key()
            .map_err(map_error_internal!("public key is invalid"))?;
        let pr_pk = decode_public_key(&pr_pk_reader);
        let stub_safety_route = SafetyRouteSpec::new();
        let sr = safety_route.unwrap_or(&stub_safety_route);
        let dh_secret = self
            .crypto
            .cached_dh(&pr_pk, &sr.secret_key)
            .map_err(map_error_internal!("dh failed"))?;
        let enc_msg_data = Crypto::encrypt(&message_data, &nonce, &dh_secret, None)
            .map_err(map_error_internal!("encryption failed"))?;

        // Prepare route operation
        let route_msg = {
            let mut route_msg = ::capnp::message::Builder::new_default();
            let mut route_operation = route_msg.init_root::<veilid_capnp::operation::Builder>();

            // Doesn't matter what this op id because there's no answer
            // but it shouldn't conflict with any other op id either
            route_operation.set_op_id(op_id);

            // Answers don't get a 'respond'
            let mut respond_to = route_operation.reborrow().init_respond_to();
            respond_to.set_none(());

            // Set up 'route' operation
            let mut route = route_operation.reborrow().init_detail().init_route();

            // Set the safety route we've constructed
            let mut msg_sr = route.reborrow().init_safety_route();
            self.encode_safety_route(sr, private_route, &mut msg_sr)?;

            // Put in the encrypted operation we're routing
            let mut msg_operation = route.init_operation();
            msg_operation.reborrow().init_signatures(0);
            let mut route_nonce = msg_operation.reborrow().init_nonce();
            encode_nonce(&nonce, &mut route_nonce);
            let data = msg_operation.reborrow().init_data(
                enc_msg_data
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("data too large"))?,
            );
            data.copy_from_slice(enc_msg_data.as_slice());

            route_msg
        };

        // Convert message to bytes and return it
        let out = builder_to_vec(route_msg)?;
        Ok(out)
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
            inner.waiting_rpc_table.remove(&op_id)
        };
        match eventual {
            None => Err(rpc_error_internal("Unmatched operation id")),
            Some(e) => Ok(e.resolve(rpcreader).await),
        }
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
                waitable_reply.node_ref.operate(|e| {
                    if waitable_reply.is_ping {
                        e.ping_lost(waitable_reply.send_ts);
                    } else {
                        e.question_lost(waitable_reply.send_ts);
                    }
                });
            }
            Ok((rpcreader, _)) => {
                // Reply received
                let recv_ts = get_timestamp();
                waitable_reply.node_ref.operate(|e| {
                    if waitable_reply.is_ping {
                        e.pong_rcvd(waitable_reply.send_ts, recv_ts, rpcreader.header.body_len);
                    } else {
                        e.answer_rcvd(waitable_reply.send_ts, recv_ts, rpcreader.header.body_len);
                    }
                });
            }
        };

        out
    }

    // Issue a request over the network, possibly using an anonymized route
    // If the request doesn't want a reply, returns immediately
    // If the request wants a reply then it waits for one asynchronously
    // If it doesn't receive a response in a sufficient time, then it returns a timeout error
    async fn request<T: capnp::message::ReaderSegments>(
        &self,
        dest: Destination,
        message: capnp::message::Reader<T>,
        safety_route_spec: Option<&SafetyRouteSpec>,
    ) -> Result<Option<WaitableReply>, RPCError> {
        let (op_id, wants_answer, is_ping) = {
            let operation = message
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_internal!("invalid operation"))?;
            let op_id = operation.get_op_id();
            let wants_answer = self.wants_answer(&operation)?;
            let is_ping = operation.get_detail().has_info_q();
            (op_id, wants_answer, is_ping)
        };

        let out_node_id;
        let mut out_noderef: Option<NodeRef> = None;
        let hopcount: usize;
        let out = {
            let out;

            // To where are we sending the request
            match dest {
                Destination::Direct(node_ref) => {
                    // Send to a node without a private route
                    // --------------------------------------
                    match safety_route_spec {
                        None => {
                            // If no safety route is being used, and we're not sending to a private
                            // route, we can use a direct envelope instead of routing
                            out = reader_to_vec(message)?;

                            // Message goes directly to the node
                            out_node_id = node_ref.node_id();
                            out_noderef = Some(node_ref);
                            hopcount = 1;
                        }
                        Some(sr) => {
                            // No private route was specified for the request
                            // but we are using a safety route, so we must create an empty private route
                            let mut pr_builder = ::capnp::message::Builder::new_default();
                            let private_route =
                                self.new_stub_private_route(node_ref.node_id(), &mut pr_builder)?;

                            let message_vec = reader_to_vec(message)?;
                            // first
                            out_node_id = sr
                                .hops
                                .first()
                                .ok_or(rpc_error_internal("no hop in safety route"))?
                                .dial_info
                                .node_id
                                .key;
                            out = self.wrap_with_route(Some(&sr), private_route, message_vec)?;
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
                    encode_private_route(&private_route, &mut pr_builder)?;
                    let pr_reader = pr_builder.into_reader();

                    // Reply with 'route' operation
                    let message_vec = reader_to_vec(message)?;
                    out_node_id = match safety_route_spec {
                        None => {
                            // If no safety route, the first node is the first hop of the private route
                            hopcount = private_route.hop_count as usize;
                            let out_node_id = match private_route.hops {
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
                                .ok_or(rpc_error_internal("no hop in safety route"))?
                                .dial_info
                                .node_id
                                .key;
                            out = self.wrap_with_route(Some(&sr), pr_reader, message_vec)?;
                            out_node_id
                        }
                    }
                }
            }
            out
        };

        // Verify hop count isn't larger than out maximum routed hop count
        if hopcount > self.inner.lock().max_route_hop_count {
            return Err(rpc_error_internal("hop count too long for route"));
        }
        // calculate actual timeout
        // timeout is number of hops times the timeout per hop
        let timeout = self.inner.lock().timeout * (hopcount as u64);

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

        // set up op id eventual
        let eventual = if wants_answer {
            Some(self.add_op_id_waiter(op_id))
        } else {
            None
        };

        // send question
        let bytes = out.len() as u64;
        if let Err(e) = self
            .network_manager()
            .send_envelope(node_ref.clone(), out)
            .await
            .map_err(|e| RPCError::Internal(e))
        {
            // Make sure to clean up op id waiter in case of error
            if eventual.is_some() {
                self.cancel_op_id_waiter(op_id);
            }
            return Err(e);
        }

        // Successfully sent
        let send_ts = get_timestamp();
        node_ref.operate(|e| {
            if is_ping {
                e.ping_sent(send_ts, bytes);
            } else {
                e.question_sent(send_ts, bytes);
            }
        });

        // Pass back waitable reply completion
        match eventual {
            None => {
                // if we don't want an answer, don't wait for one
                Ok(None)
            }
            Some(e) => Ok(Some(WaitableReply {
                op_id: op_id,
                eventual: e,
                timeout: timeout,
                node_ref: node_ref,
                send_ts: send_ts,
                is_ping: is_ping,
            })),
        }
    }

    // Issue a reply over the network, possibly using an anonymized route
    // If the request doesn't want a reply, this routine does nothing
    async fn reply<T: capnp::message::ReaderSegments>(
        &self,
        request_rpcreader: RPCMessageReader,
        reply_msg: capnp::message::Reader<T>,
        safety_route_spec: Option<&SafetyRouteSpec>,
    ) -> Result<(), RPCError> {
        //
        let out_node_id;
        let mut out_noderef: Option<NodeRef> = None;
        let is_pong = {
            let operation = reply_msg
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_internal!("invalid operation"))?;
            operation.get_detail().has_info_a()
        };

        let out = {
            let out;

            let request_operation = request_rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())?;

            let reply_vec = reader_to_vec(reply_msg)?;

            // To where should we respond?
            match request_operation
                .get_respond_to()
                .which()
                .map_err(map_error_internal!("invalid request operation"))?
            {
                veilid_capnp::operation::respond_to::None(_) => {
                    // Do not respond
                    // --------------
                    return Ok(());
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
                            let private_route = self.new_stub_private_route(
                                request_rpcreader.header.envelope.get_sender_id(),
                                &mut pr_builder,
                            )?;

                            out = self.wrap_with_route(Some(sr), private_route, reply_vec)?;
                            // first
                            out_node_id = sr
                                .hops
                                .first()
                                .ok_or(rpc_error_internal("no hop in safety route"))?
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
                        Err(_) => return Err(rpc_error_internal("invalid private route")),
                    };

                    // Reply with 'route' operation
                    out =
                        self.wrap_with_route(safety_route_spec.clone(), private_route, reply_vec)?;
                    out_node_id = match safety_route_spec {
                        None => {
                            // If no safety route, the first node is the first hop of the private route
                            if !private_route.has_first_hop() {
                                return Err(rpc_error_internal("private route has no hops"));
                            }
                            let hop = private_route
                                .get_first_hop()
                                .map_err(map_error_internal!("not a valid first hop"))?;
                            decode_public_key(
                                &hop.get_dial_info()
                                    .map_err(map_error_internal!("not a valid dial info"))?
                                    .get_node_id()
                                    .map_err(map_error_internal!("not a valid node id"))?,
                            )
                        }
                        Some(sr) => {
                            // If safety route is in use, first node is the first hop of the safety route
                            sr.hops
                                .first()
                                .ok_or(rpc_error_internal("no hop in safety route"))?
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
            .send_envelope(node_ref.clone(), out)
            .await
            .map_err(|e| RPCError::Internal(e))?;

        // Reply successfully sent
        let send_ts = get_timestamp();

        node_ref.operate(|e| {
            if is_pong {
                e.pong_sent(send_ts, bytes);
            } else {
                e.answer_sent(send_ts, bytes);
            }
        });

        Ok(())
    }

    fn wants_answer(&self, request: &veilid_capnp::operation::Reader) -> Result<bool, RPCError> {
        match request.get_respond_to().which() {
            Ok(veilid_capnp::operation::respond_to::None(_)) => Ok(false),
            Ok(veilid_capnp::operation::respond_to::Sender(_)) => Ok(true),
            Ok(veilid_capnp::operation::respond_to::PrivateRoute(_)) => Ok(true),
            _ => Err(rpc_error_internal("Unknown respond_to")),
        }
    }

    fn can_validate_dial_info(&self) -> bool {
        let nman = self.network_manager();
        match nman.get_network_class() {
            NetworkClass::Server => true,
            NetworkClass::Mapped => true,
            NetworkClass::FullNAT => true,
            NetworkClass::AddressRestrictedNAT => false,
            NetworkClass::PortRestrictedNAT => false,
            NetworkClass::OutboundOnly => false,
            NetworkClass::WebApp => false,
            NetworkClass::TorWebApp => false,
            NetworkClass::Invalid => false,
        }
    }

    fn will_validate_dial_info(&self) -> bool {
        if !self.can_validate_dial_info() {
            return false;
        }

        // only accept info redirects if we aren't using a relay lease
        // which means our dial info refers to our own actual ip address and not some other node
        let nman = self.network_manager();
        let lman = nman.lease_manager();
        if lman.client_get_relay_mode() != RelayMode::Disabled {
            return false;
        }
        // xxx: bandwidth limiting here, don't commit to doing info redirects if our network quality sucks

        return true;
    }
    //////////////////////////////////////////////////////////////////////

    fn generate_node_info(&self) -> NodeInfo {
        let nman = self.network_manager();
        let lman = nman.lease_manager();

        let can_route = false; // xxx: until we implement this we dont have accounting for it
        let will_route = false;
        let can_tunnel = false; // xxx: until we implement this we dont have accounting for it
        let will_tunnel = false;
        let can_signal_lease = lman.server_can_provide_signal_lease();
        let will_signal_lease = lman.server_will_provide_signal_lease();
        let can_relay_lease = lman.server_can_provide_relay_lease();
        let will_relay_lease = lman.server_will_provide_relay_lease();
        let can_validate_dial_info = self.can_validate_dial_info();
        let will_validate_dial_info = self.will_validate_dial_info();

        NodeInfo {
            can_route: can_route,
            will_route: will_route,
            can_tunnel: can_tunnel,
            will_tunnel: will_tunnel,
            can_signal_lease: can_signal_lease,
            will_signal_lease: will_signal_lease,
            can_relay_lease: can_relay_lease,
            will_relay_lease: will_relay_lease,
            can_validate_dial_info: can_validate_dial_info,
            will_validate_dial_info: will_validate_dial_info,
        }
    }

    fn generate_sender_info(&self, rpcreader: &RPCMessageReader) -> SenderInfo {
        let socket_address =
            rpcreader
                .header
                .peer_noderef
                .operate(|entry| match entry.last_connection() {
                    None => None,
                    Some(c) => c.remote.to_socket_addr().ok(),
                });
        SenderInfo {
            socket_address: socket_address,
        }
    }

    async fn process_info_q(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        //
        let reply_msg = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())?;

            // Don't bother unless we are going to answer
            if !self.wants_answer(&operation)? {
                return Ok(());
            }

            // Send info answer
            let mut reply_msg = ::capnp::message::Builder::new_default();
            let mut answer = reply_msg.init_root::<veilid_capnp::operation::Builder>();
            answer.set_op_id(operation.get_op_id());
            let mut respond_to = answer.reborrow().init_respond_to();
            respond_to.set_none(());
            let detail = answer.reborrow().init_detail();
            let mut info_a = detail.init_info_a();
            // Add node info
            let node_info = self.generate_node_info();
            let mut nib = info_a.reborrow().init_node_info();
            encode_node_info(&node_info, &mut nib)?;
            // Add sender info
            let sender_info = self.generate_sender_info(&rpcreader);
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
                .map_err(map_error_capnp_error!())?;

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
            let protocol_address_type = dial_info.protocol_address_type();
            let peers = routing_table.get_fast_nodes_of_type(protocol_address_type);
            if peers.len() == 0 {
                return Err(rpc_error_internal(format!(
                    "no peers of type '{:?}'",
                    protocol_address_type
                )));
            }

            for peer in peers {
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
            .send_direct_receipt(dial_info, rcpt_data, alternate_port)
            .await
            .map_err(map_error_string!())?;

        Ok(())
    }

    async fn process_find_node_q(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        //
        let reply_msg = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())?;

            // find_node must always want an answer
            if !self.wants_answer(&operation)? {
                return Err(RPCError::InvalidFormat);
            }

            // get findNodeQ reader
            let fnq_reader = match operation.get_detail().which() {
                Ok(veilid_capnp::operation::detail::Which::FindNodeQ(Ok(x))) => x,
                _ => panic!("invalid operation type in process_find_node_q"),
            };

            // ensure find_node peerinfo matches the envelope
            let target_node_id =
                decode_public_key(&fnq_reader.get_node_id().map_err(map_error_capnp_error!())?);
            let peer_info = decode_peer_info(
                &fnq_reader
                    .get_peer_info()
                    .map_err(map_error_capnp_error!())?,
            )?;
            if peer_info.node_id.key != rpcreader.header.envelope.get_sender_id() {
                return Err(RPCError::InvalidFormat);
            }

            // filter out attempts to pass non-public addresses in for peers
            let address_filter = self.config.get().network.address_filter;
            if address_filter {
                for di in &peer_info.dial_infos {
                    if !di.is_public().map_err(map_error_string!())? {
                        // non-public address causes rejection
                        return Err(RPCError::InvalidFormat);
                    }
                }
            }

            // add node information for the requesting node to our routing table
            let routing_table = self.routing_table();
            let _ = routing_table
                .register_node_with_dial_info(peer_info.node_id.key, &peer_info.dial_infos)
                .map_err(map_error_string!())?;

            // find N nodes closest to the target node in our routing table
            let peer_scope = if address_filter {
                PeerScope::Public
            } else {
                PeerScope::All
            };
            let own_peer_info = routing_table.get_own_peer_info(peer_scope);
            let closest_nodes = routing_table.find_closest_nodes(
                target_node_id,
                // filter
                None,
                // transform
                |e| RoutingTable::transform_to_peer_info(e, peer_scope, &own_peer_info),
            );
            trace!(">>>> Returning {} closest peers", closest_nodes.len());

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
            for i in 0..closest_nodes.len() {
                let mut pi_builder = peers_builder.reborrow().get(i as u32);
                encode_peer_info(&closest_nodes[i], &mut pi_builder)?;
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

    async fn process_signal_q(&self, _rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        Err(rpc_error_unimplemented("process_signal_q"))
    }

    async fn process_return_receipt(&self, rpcreader: RPCMessageReader) -> Result<(), RPCError> {
        let rcpt_data = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())?;

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
            .process_receipt(rcpt_data)
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
                .map_err(map_error_capnp_error!())?;
            operation.get_op_id()
        };

        Ok(self.complete_op_id_waiter(op_id, rpcreader).await?)
    }

    //////////////////////////////////////////////////////////////////////

    async fn process_rpc_message_version_0(&self, msg: RPCMessage) -> Result<(), RPCError> {
        let reader = capnp::message::Reader::new(msg.data, Default::default());
        let rpcreader = RPCMessageReader {
            header: msg.header,
            reader: reader,
        };

        let (which, is_q) = {
            let operation = rpcreader
                .reader
                .get_root::<veilid_capnp::operation::Reader>()
                .map_err(map_error_capnp_error!())?;

            match operation
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
                veilid_capnp::operation::detail::SignalQ(_) => (17u32, true),
                veilid_capnp::operation::detail::SignalA(_) => (18u32, false),
                veilid_capnp::operation::detail::ReturnReceipt(_) => (19u32, true),
                veilid_capnp::operation::detail::StartTunnelQ(_) => (20u32, true),
                veilid_capnp::operation::detail::StartTunnelA(_) => (21u32, false),
                veilid_capnp::operation::detail::CompleteTunnelQ(_) => (22u32, true),
                veilid_capnp::operation::detail::CompleteTunnelA(_) => (23u32, false),
                veilid_capnp::operation::detail::CancelTunnelQ(_) => (24u32, true),
                veilid_capnp::operation::detail::CancelTunnelA(_) => (25u32, false),
            }
        };
        // Accounting for questions we receive
        if is_q {
            // look up sender node, in case it's different than our peer due to relaying
            if let Some(sender_nr) = self
                .routing_table()
                .lookup_node_ref(rpcreader.header.envelope.get_sender_id())
            {
                if which == 0u32 {
                    sender_nr.operate(|e| {
                        e.ping_rcvd(rpcreader.header.timestamp, rpcreader.header.body_len);
                    });
                } else {
                    sender_nr.operate(|e| {
                        e.question_rcvd(rpcreader.header.timestamp, rpcreader.header.body_len);
                    });
                }
            }
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
            17 => self.process_signal_q(rpcreader).await, // SignalQ
            18 => self.process_answer(rpcreader).await, // SignalA
            19 => self.process_return_receipt(rpcreader).await, // ReturnReceipt
            20 => self.process_start_tunnel_q(rpcreader).await, // StartTunnelQ
            21 => self.process_answer(rpcreader).await, // StartTunnelA
            22 => self.process_complete_tunnel_q(rpcreader).await, // CompleteTunnelQ
            23 => self.process_answer(rpcreader).await, // CompleteTunnelA
            24 => self.process_cancel_tunnel_q(rpcreader).await, // CancelTunnelQ
            25 => self.process_answer(rpcreader).await, // CancelTunnelA
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

    async fn rpc_worker(self, receiver: Receiver<RPCMessage>) {
        loop {
            let msg = match receiver.recv().await {
                Ok(v) => v,
                Err(_) => {
                    break;
                }
            };

            match self.process_rpc_message(msg).await {
                Ok(_) => (),
                Err(e) => {
                    error!("Couldn't process rpc message: {}", e);
                }
            }
        }
    }

    pub async fn startup(&self) -> Result<(), String> {
        trace!("VeilidCore::startup init RPC processor");
        let mut inner = self.inner.lock();
        // make local copy of node id for easy access
        let c = self.config.get();
        inner.node_id = c.network.node_id;
        inner.node_id_secret = c.network.node_id_secret;

        // set up channel
        let mut concurrency = c.network.rpc.concurrency;
        let mut queue_size = c.network.rpc.queue_size;
        let mut timeout = c.network.rpc.timeout;
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
        let channel = channel(queue_size as usize);
        inner.channel = Some(channel.clone());

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

    pub async fn enqueue_message(
        &self,
        envelope: envelope::Envelope,
        body: Vec<u8>,
        peer_noderef: NodeRef,
    ) -> Result<(), ()> {
        debug!("enqueue_message: body len = {}", body.len());
        let msg = RPCMessage {
            header: RPCMessageHeader {
                timestamp: get_timestamp(),
                envelope: envelope,
                body_len: body.len() as u64,
                peer_noderef: peer_noderef,
            },
            data: RPCMessageData { contents: body },
        };
        let send_channel = {
            let inner = self.inner.lock();
            inner.channel.as_ref().unwrap().0.clone()
        };
        send_channel.try_send(msg).await.map_err(drop)?;
        Ok(())
    }

    // Send InfoQ RPC request, receive InfoA answer
    pub async fn rpc_call_info(self, peer: NodeRef) -> Result<InfoAnswer, RPCError> {
        let info_q_msg = {
            let mut info_q_msg = ::capnp::message::Builder::new_default();
            let mut question = info_q_msg.init_root::<veilid_capnp::operation::Builder>();
            question.set_op_id(self.get_next_op_id());
            let mut respond_to = question.reborrow().init_respond_to();
            respond_to.set_sender(());
            let detail = question.reborrow().init_detail();
            let _ = detail.init_info_q();

            info_q_msg.into_reader()
        };

        // Send the info request
        let waitable_reply = self
            .request(Destination::Direct(peer.clone()), info_q_msg, None)
            .await?
            .unwrap();

        // Wait for reply
        let (rpcreader, latency) = self.wait_for_reply(waitable_reply).await?;

        let response_operation = rpcreader
            .reader
            .get_root::<veilid_capnp::operation::Reader>()
            .map_err(map_error_capnp_error!())?;
        let info_a = match response_operation
            .get_detail()
            .which()
            .map_err(map_error_capnp_notinschema!())?
        {
            veilid_capnp::operation::detail::InfoA(a) => {
                a.map_err(map_error_internal!("Invalid InfoA"))?
            }
            _ => return Err(rpc_error_internal("Incorrect RPC answer for question")),
        };

        // Decode node info
        if !info_a.has_node_info() {
            return Err(rpc_error_internal("Missing node info"));
        }
        let nir = info_a
            .get_node_info()
            .map_err(map_error_internal!("Broken node info"))?;
        let node_info = decode_node_info(&nir)?;

        // Decode sender info
        let sender_info = if info_a.has_sender_info() {
            let sir = info_a
                .get_sender_info()
                .map_err(map_error_internal!("Broken sender info"))?;
            decode_sender_info(&sir)?
        } else {
            SenderInfo::default()
        };

        // Update latest node info in routing table
        peer.operate(|e| {
            e.update_node_info(node_info.clone());
        });

        // Return the answer for anyone who may care
        let out = InfoAnswer {
            latency: latency,
            node_info: node_info,
            sender_info: sender_info,
        };

        Ok(out)
    }

    pub async fn rpc_call_validate_dial_info(
        &self,
        peer: NodeRef,
        dial_info: DialInfo,
        redirect: bool,
        alternate_port: bool,
    ) -> Result<bool, RPCError> {
        let network_manager = self.network_manager();
        let receipt_time = self
            .config
            .get()
            .network
            .dht
            .validate_dial_info_receipt_time;
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
        self.request(Destination::Direct(peer.clone()), vdi_msg, None)
            .await?;

        // Wait for receipt
        match eventual_value.await {
            ReceiptEvent::RETURNED => Ok(true),
            ReceiptEvent::EXPIRED => Ok(false),
            ReceiptEvent::CANCELLED => Err(rpc_error_internal(
                "receipt was dropped before expiration".to_owned(),
            )),
        }
    }

    // Send FindNodeQ RPC request, receive FindNodeA answer
    pub async fn rpc_call_find_node(
        self,
        dest: Destination,
        key: key::DHTKey,
        safety_route: Option<&SafetyRouteSpec>,
        respond_to: RespondTo,
    ) -> Result<FindNodeAnswer, RPCError> {
        let address_filter = self.config.get().network.address_filter;
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
            let mut peer_info_builder = fnq.reborrow().init_peer_info();

            let own_peer_info = self.routing_table().get_own_peer_info(if address_filter {
                PeerScope::Public
            } else {
                PeerScope::All
            });
            if own_peer_info.dial_infos.len() == 0 {
                return Err(rpc_error_internal("No valid public dial info for own node"));
            }

            encode_peer_info(&own_peer_info, &mut peer_info_builder)?;

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
            .map_err(map_error_capnp_error!())?;
        let find_node_a = match response_operation
            .get_detail()
            .which()
            .map_err(map_error_capnp_notinschema!())?
        {
            veilid_capnp::operation::detail::FindNodeA(a) => {
                a.map_err(map_error_internal!("Invalid FindNodeA"))?
            }
            _ => return Err(rpc_error_internal("Incorrect RPC answer for question")),
        };

        let peers_reader = find_node_a
            .get_peers()
            .map_err(map_error_internal!("Missing peers"))?;
        let mut peers_vec = Vec::<PeerInfo>::with_capacity(
            peers_reader
                .len()
                .try_into()
                .map_err(map_error_internal!("too many peers"))?,
        );
        for p in peers_reader.iter() {
            let peer_info = decode_peer_info(&p)?;

            // reject attempts to include non-public addresses in results
            if address_filter {
                for di in &peer_info.dial_infos {
                    if !di.is_public().map_err(map_error_string!())? {
                        // non-public address causes rejection
                        return Err(RPCError::InvalidFormat);
                    }
                }
            }

            peers_vec.push(peer_info);
        }

        let out = FindNodeAnswer {
            latency: latency,
            peers: peers_vec,
        };

        Ok(out)
    }

    // xxx do not process latency for routed messages
}
