use super::*;
use futures_util::{FutureExt, StreamExt};
use std::{io, sync::Arc};
use stop_token::prelude::*;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        // No accept support for WASM
    } else {

        ///////////////////////////////////////////////////////////
        // Accept

        pub(in crate::network_manager) trait ProtocolAcceptHandler: ProtocolAcceptHandlerClone + Send + Sync {
            fn on_accept(
                &self,
                stream: AsyncPeekStream,
                peer_addr: SocketAddr,
                local_addr: SocketAddr,
            ) -> SendPinBoxFuture<io::Result<Option<ProtocolNetworkConnection>>>;
        }

        pub(in crate::network_manager)  trait ProtocolAcceptHandlerClone {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler>;
        }

        impl<T> ProtocolAcceptHandlerClone for T
        where
            T: 'static + ProtocolAcceptHandler + Clone,
        {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler> {
                Box::new(self.clone())
            }
        }
        impl Clone for Box<dyn ProtocolAcceptHandler> {
            fn clone(&self) -> Box<dyn ProtocolAcceptHandler> {
                self.clone_box()
            }
        }

        pub(in crate::network_manager) type NewProtocolAcceptHandler =
            dyn Fn(VeilidConfig, bool) -> Box<dyn ProtocolAcceptHandler> + Send;
    }
}
///////////////////////////////////////////////////////////
// Dummy protocol network connection for testing

// #[derive(Debug)]
// pub struct DummyNetworkConnection {
//     flow: Flow,
// }

// impl DummyNetworkConnection {
//     pub fn flow(&self) -> Flow {
//         self.flow
//     }
//     pub fn close(&self) -> io::Result<NetworkResult<()>> {
//         Ok(NetworkResult::Value(()))
//     }
//     pub fn send(&self, _message: Vec<u8>) -> io::Result<NetworkResult<()>> {
//         Ok(NetworkResult::Value(()))
//     }
//     pub fn recv(&self) -> io::Result<NetworkResult<Vec<u8>>> {
//         Ok(NetworkResult::Value(Vec::new()))
//     }
// }

///////////////////////////////////////////////////////////
// Top-level protocol independent network connection object

#[derive(Clone, Copy, Debug)]
enum RecvLoopAction {
    Send,
    Recv,
    Finish,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct NetworkConnectionStats {
    last_message_sent_time: Option<Timestamp>,
    last_message_recv_time: Option<Timestamp>,
}

#[derive(Debug)]
pub(in crate::network_manager) struct NetworkConnection {
    connection_id: NetworkConnectionId,
    flow: Flow,
    processor: Option<MustJoinHandle<()>>,
    established_time: Timestamp,
    stats: Arc<Mutex<NetworkConnectionStats>>,
    sender: flume::Sender<(Option<Id>, Vec<u8>)>,
    stop_source: Option<StopSource>,
    protected_nr: Option<NodeRef>,
    ref_count: usize,
}

impl Drop for NetworkConnection {
    fn drop(&mut self) {
        if self.ref_count != 0 && self.stop_source.is_some() {
            log_net!(error "ref_count for network connection should be zero: {:?}", self);
        }
    }
}

impl NetworkConnection {
    pub(super) fn dummy(id: NetworkConnectionId, flow: Flow) -> Self {
        // Create handle for sending (dummy is immediately disconnected)
        let (sender, _receiver) = flume::bounded(get_concurrency() as usize);

        Self {
            connection_id: id,
            flow,
            processor: None,
            established_time: Timestamp::now(),
            stats: Arc::new(Mutex::new(NetworkConnectionStats {
                last_message_sent_time: None,
                last_message_recv_time: None,
            })),
            sender,
            stop_source: None,
            protected_nr: None,
            ref_count: 0,
        }
    }

    pub(super) fn from_protocol(
        connection_manager: ConnectionManager,
        manager_stop_token: StopToken,
        protocol_connection: ProtocolNetworkConnection,
        connection_id: NetworkConnectionId,
    ) -> Self {
        // Get flow
        let flow = protocol_connection.flow();

        // Create handle for sending
        //let (sender, receiver) = flume::bounded(get_concurrency() as usize);
        let (sender, receiver) = flume::unbounded();

        // Create stats
        let stats = Arc::new(Mutex::new(NetworkConnectionStats {
            last_message_sent_time: None,
            last_message_recv_time: None,
        }));

        let stop_source = StopSource::new();
        let local_stop_token = stop_source.token();

        // Spawn connection processor and pass in protocol connection
        let processor = spawn(
            "connection processor",
            Self::process_connection(
                connection_manager,
                local_stop_token,
                manager_stop_token,
                connection_id,
                flow,
                receiver,
                protocol_connection,
                stats.clone(),
            ),
        );

        // Return the connection
        Self {
            connection_id,
            flow,
            processor: Some(processor),
            established_time: Timestamp::now(),
            stats,
            sender,
            stop_source: Some(stop_source),
            protected_nr: None,
            ref_count: 0,
        }
    }

    pub fn connection_id(&self) -> NetworkConnectionId {
        self.connection_id
    }

    pub fn flow(&self) -> Flow {
        self.flow
    }

    #[expect(dead_code)]
    pub fn unique_flow(&self) -> UniqueFlow {
        UniqueFlow {
            flow: self.flow,
            connection_id: Some(self.connection_id),
        }
    }

    pub fn get_handle(&self) -> ConnectionHandle {
        ConnectionHandle::new(self.connection_id, self.flow, self.sender.clone())
    }

    pub fn is_in_use(&self) -> bool {
        self.ref_count > 0
    }

    pub fn protected_node_ref(&self) -> Option<NodeRef> {
        self.protected_nr.clone()
    }

    pub fn protect(&mut self, protect_nr: NodeRef) {
        self.protected_nr = Some(protect_nr);
    }

    pub fn unprotect(&mut self) {
        self.protected_nr = None;
    }

    pub fn add_ref(&mut self) {
        self.ref_count += 1;
    }

    pub fn remove_ref(&mut self) {
        self.ref_count -= 1;
    }

    pub fn close(&mut self) {
        if let Some(stop_source) = self.stop_source.take() {
            // drop the stopper
            drop(stop_source);
        }
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    async fn send_internal(
        protocol_connection: &ProtocolNetworkConnection,
        stats: Arc<Mutex<NetworkConnectionStats>>,
        message: Vec<u8>,
    ) -> io::Result<NetworkResult<()>> {
        let ts = Timestamp::now();
        network_result_try!(protocol_connection.send(message).await?);

        let mut stats = stats.lock();
        stats.last_message_sent_time.max_assign(Some(ts));

        Ok(NetworkResult::Value(()))
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    async fn recv_internal(
        protocol_connection: &ProtocolNetworkConnection,
        stats: Arc<Mutex<NetworkConnectionStats>>,
    ) -> io::Result<NetworkResult<Vec<u8>>> {
        let ts = Timestamp::now();
        let out = network_result_try!(protocol_connection.recv().await?);

        let mut stats = stats.lock();
        stats.last_message_recv_time.max_assign(Some(ts));

        #[cfg(feature = "verbose-tracing")]
        tracing::Span::current().record("ret.len", out.len());

        Ok(NetworkResult::Value(out))
    }

    pub fn stats(&self) -> NetworkConnectionStats {
        let stats = self.stats.lock();
        stats.clone()
    }

    #[expect(dead_code)]
    pub fn established_time(&self) -> Timestamp {
        self.established_time
    }

    // Connection receiver loop
    #[allow(clippy::too_many_arguments)]
    #[instrument(parent = None, level="trace", target="net", skip_all)]
    fn process_connection(
        connection_manager: ConnectionManager,
        local_stop_token: StopToken,
        manager_stop_token: StopToken,
        connection_id: NetworkConnectionId,
        flow: Flow,
        receiver: flume::Receiver<(Option<Id>, Vec<u8>)>,
        protocol_connection: ProtocolNetworkConnection,
        stats: Arc<Mutex<NetworkConnectionStats>>,
    ) -> SendPinBoxFuture<()> {
        Box::pin(async move {
            log_net!(
                "Starting process_connection loop for id={}, {:?}", connection_id,
                flow
            );

            let network_manager = connection_manager.network_manager();
            let address_filter = network_manager.address_filter();
            let mut unord = FuturesUnordered::new();
            let mut need_receiver = true;
            let mut need_sender = true;

            // Push mutable timer so we can reset it
            // Normally we would use an io::timeout here, but WASM won't support that, so we use a mutable sleep future
            let new_timer = || {
                sleep(connection_manager.connection_inactivity_timeout_ms()).then(|_| async {
                    // timeout
                    log_net!("Connection timeout on {:?}", flow);
                    RecvLoopAction::Timeout
                })
            };
            let timer = MutableFuture::new(new_timer());

            unord.push(system_boxed(timer.clone().in_current_span()));

            loop {
                // Add another message sender future if necessary
                if need_sender {
                    need_sender = false;
                    let sender_fut = receiver.recv_async().then(|res| async {
                        match res {
                            Ok((_span_id, message)) => {
                                // Touch the LRU for this connection
                                connection_manager.touch_connection_by_id(connection_id);

                                // send the packet
                                if let Err(e) = Self::send_internal(
                                    &protocol_connection,
                                    stats.clone(),
                                    message,
                                )
                                .await
                                {
                                    // Sending the packet along can fail, if so, this connection is dead
                                    log_net!(debug e);
                                    RecvLoopAction::Finish
                                } else {
                                    RecvLoopAction::Send
                                }
                            }
                            Err(e) => {
                                // All senders gone, shouldn't happen since we store one alongside the join handle
                                log_net!(warn e);
                                RecvLoopAction::Finish
                            }
                        }
                    }.in_current_span());
                    unord.push(system_boxed(sender_fut.in_current_span()));
                }

                // Add another message receiver future if necessary
                if need_receiver {
                    need_receiver = false;
                    let receiver_fut = Self::recv_internal(&protocol_connection, stats.clone())
                        .then(|res| async {
                            match res {
                                Ok(v) => {
                                    let peer_address = protocol_connection.flow().remote();

                                    // Check to see if it is punished
                                    if address_filter.is_ip_addr_punished(peer_address.socket_addr().ip()) {
                                        return RecvLoopAction::Finish;
                                    }

                                    // Check for connection close
                                    if v.is_no_connection() {
                                        log_net!("Connection closed from: {} ({})", peer_address.socket_addr(), peer_address.protocol_type());
                                        return RecvLoopAction::Finish;
                                    }

                                    // Punish invalid framing (tcp framing or websocket framing)
                                    if v.is_invalid_message() {
                                        address_filter.punish_ip_addr(peer_address.socket_addr().ip(), PunishmentReason::InvalidFraming);
                                        return RecvLoopAction::Finish;
                                    }

                                    // Log other network results
                                    let mut message = network_result_value_or_log!(v => [ format!(": protocol_connection={:?}", protocol_connection) ] {
                                        return RecvLoopAction::Finish;
                                    });

                                    // Pass received messages up to the network manager for processing
                                    if let Err(e) = network_manager
                                        .on_recv_envelope(message.as_mut_slice(), flow)
                                        .await
                                    {
                                        log_net!(debug "failed to process received envelope: {}", e);
                                        RecvLoopAction::Finish
                                    } else {
                                        // Touch the LRU for this connection
                                        connection_manager.touch_connection_by_id(connection_id);

                                        RecvLoopAction::Recv
                                    }
                                }
                                Err(e) => {
                                    // Connection unable to receive, closed
                                    log_net!(error "connection unable to receive: {}", e);
                                    RecvLoopAction::Finish
                                }
                            }
                        }.in_current_span());

                    unord.push(system_boxed(receiver_fut.in_current_span()));
                }

                // Process futures
                match unord
                    .next()
                    .timeout_at(local_stop_token.clone())
                    .timeout_at(manager_stop_token.clone())
                    .await
                    .and_then(std::convert::identity)   // flatten stoptoken timeouts
                {
                    Ok(Some(RecvLoopAction::Send)) => {
                        // Don't reset inactivity timer if we're only sending
                        need_sender = true;
                    }
                    Ok(Some(RecvLoopAction::Recv)) => {
                        // Reset inactivity timer since we got something from this connection
                        timer.set(new_timer());

                        need_receiver = true;
                    }
                    Ok(Some(RecvLoopAction::Finish) | Some(RecvLoopAction::Timeout)) => {
                        break;
                    }
                    Ok(None) => {
                        // Should not happen
                        unreachable!();
                    }
                    Err(_) => {
                        // Either one of the stop tokens
                        break;
                    }
                }
            }

            log_net!(
                "Connection loop finished flow={:?}",
                flow
            );

            // Let the connection manager know the receive loop exited
            connection_manager
                .report_connection_finished(connection_id)
                .await;

            // Close the low level socket
            if let Err(e) = protocol_connection.close().await {
                log_net!(debug "Protocol connection close error: {}", e);
            }
        }.in_current_span())
    }

    pub fn debug_print(&self, cur_ts: Timestamp) -> String {
        format!(
            "{} <- {} | {} | est {} sent {} rcvd {} refcount {}{}",
            self.flow.remote_address(),
            self.flow
                .local()
                .map(|x| x.to_string())
                .unwrap_or("---".to_owned()),
            self.connection_id.as_u64(),
            debug_duration(
                cur_ts
                    .as_u64()
                    .saturating_sub(self.established_time.as_u64())
            ),
            self.stats()
                .last_message_sent_time
                .map(|ts| debug_duration(cur_ts.as_u64().saturating_sub(ts.as_u64())))
                .unwrap_or("---".to_owned()),
            self.stats()
                .last_message_recv_time
                .map(|ts| debug_duration(cur_ts.as_u64().saturating_sub(ts.as_u64())))
                .unwrap_or("---".to_owned()),
            self.ref_count,
            if let Some(pnr) = &self.protected_nr {
                format!(" PROTECTED:{}", pnr)
            } else {
                "".to_owned()
            },
        )
    }
}

// Resolves ready when the connection loop has terminated
impl Future for NetworkConnection {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let mut pending = 0usize;

        // Process all sub-futures, nulling them out when they return ready
        if let Some(mut processor) = self.processor.as_mut() {
            if Pin::new(&mut processor).poll(cx).is_ready() {
                self.processor = None;
            } else {
                pending += 1
            }
        }

        // Any sub-futures pending?
        if pending > 0 {
            task::Poll::Pending
        } else {
            task::Poll::Ready(())
        }
    }
}
