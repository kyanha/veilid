use super::*;

/// The context of the outbound_watch_value operation
struct OutboundWatchValueContext {
    /// A successful watch
    pub opt_watch_value_result: Option<OutboundWatchValueResult>,
}

/// The result of the outbound_watch_value operation
#[derive(Debug, Clone)]
pub(super) struct OutboundWatchValueResult {
    /// The expiration of a successful watch
    pub expiration_ts: Timestamp,
    /// What watch id was returned
    pub watch_id: u64,
    /// Which node accepted the watch
    pub watch_node: NodeRef,
    /// Which private route is responsible for receiving ValueChanged notifications
    pub opt_value_changed_route: Option<PublicKey>,
}

impl StorageManager {
    /// Perform a 'watch value cancel' on the network without fanout
    #[allow(clippy::too_many_arguments)]
    #[instrument(level = "trace", target = "dht", skip_all, err)]
    pub(super) async fn outbound_watch_value_cancel(
        &self,
        rpc_processor: RPCProcessor,
        key: TypedKey,
        subkeys: ValueSubkeyRangeSet,
        safety_selection: SafetySelection,
        opt_watcher: Option<KeyPair>,
        watch_id: u64,
        watch_node: NodeRef,
    ) -> VeilidAPIResult<Option<OutboundWatchValueResult>> {
        // Get the appropriate watcher key, if anonymous use a static anonymous watch key
        // which lives for the duration of the app's runtime
        let watcher = opt_watcher.unwrap_or_else(|| {
            self.unlocked_inner
                .anonymous_watch_keys
                .get(key.kind)
                .unwrap()
                .value
        });

        let wva = VeilidAPIError::from_network_result(
            rpc_processor
                .clone()
                .rpc_call_watch_value(
                    Destination::direct(watch_node.clone()).with_safety(safety_selection),
                    key,
                    subkeys,
                    Timestamp::default(),
                    0,
                    watcher,
                    Some(watch_id),
                )
                .await?,
        )?;

        if wva.answer.accepted {
            log_dht!(debug "WatchValue canceled: id={} expiration_ts={} ({})", wva.answer.watch_id, debug_ts(wva.answer.expiration_ts.as_u64()), watch_node);
            Ok(Some(OutboundWatchValueResult {
                expiration_ts: wva.answer.expiration_ts,
                watch_id: wva.answer.watch_id,
                watch_node,
                opt_value_changed_route: wva.reply_private_route,
            }))
        } else {
            log_dht!(debug "WatchValue not canceled: id={} ({})", watch_id, watch_node);
            Ok(None)
        }
    }

    /// Perform a 'watch value change' on the network without fanout
    #[allow(clippy::too_many_arguments)]
    #[instrument(target = "dht", level = "debug", skip_all, err)]
    pub(super) async fn outbound_watch_value_change(
        &self,
        rpc_processor: RPCProcessor,
        key: TypedKey,
        subkeys: ValueSubkeyRangeSet,
        expiration: Timestamp,
        count: u32,
        safety_selection: SafetySelection,
        opt_watcher: Option<KeyPair>,
        watch_id: u64,
        watch_node: NodeRef,
    ) -> VeilidAPIResult<Option<OutboundWatchValueResult>> {
        if count == 0 {
            apibail_internal!("cancel should be done with outbound_watch_value_cancel");
        }
        if watch_id == 0 {
            apibail_internal!("watch id should not be zero when changing watch");
        }

        // Get the appropriate watcher key, if anonymous use a static anonymous watch key
        // which lives for the duration of the app's runtime
        let watcher = opt_watcher.unwrap_or_else(|| {
            self.unlocked_inner
                .anonymous_watch_keys
                .get(key.kind)
                .unwrap()
                .value
        });

        let wva = VeilidAPIError::from_network_result(
            rpc_processor
                .clone()
                .rpc_call_watch_value(
                    Destination::direct(watch_node.clone()).with_safety(safety_selection),
                    key,
                    subkeys,
                    expiration,
                    count,
                    watcher,
                    Some(watch_id),
                )
                .await?,
        )?;

        if wva.answer.accepted {
            if watch_id != wva.answer.watch_id {
                log_dht!(debug "WatchValue changed: id={}->{} expiration_ts={} ({})", watch_id, wva.answer.watch_id, debug_ts(wva.answer.expiration_ts.as_u64()), watch_node);
            } else {
                log_dht!(debug "WatchValue renewed: id={} expiration_ts={} ({})", watch_id, debug_ts(wva.answer.expiration_ts.as_u64()), watch_node);
            }

            Ok(Some(OutboundWatchValueResult {
                expiration_ts: wva.answer.expiration_ts,
                watch_id: wva.answer.watch_id,
                watch_node,
                opt_value_changed_route: wva.reply_private_route,
            }))
        } else {
            log_dht!(debug "WatchValue change failed: id={} ({})", wva.answer.watch_id, watch_node);
            Ok(None)
        }
    }

    /// Perform a 'watch value' query on the network using fanout
    #[allow(clippy::too_many_arguments)]
    #[instrument(level = "trace", target = "dht", skip_all, err)]
    pub(super) async fn outbound_watch_value(
        &self,
        rpc_processor: RPCProcessor,
        key: TypedKey,
        subkeys: ValueSubkeyRangeSet,
        expiration: Timestamp,
        count: u32,
        safety_selection: SafetySelection,
        opt_watcher: Option<KeyPair>,
        opt_watch_id: Option<u64>,
        opt_watch_node: Option<NodeRef>,
    ) -> VeilidAPIResult<Option<OutboundWatchValueResult>> {
        // if the count is zero, we are cancelling
        if count == 0 {
            // Ensure watch id is specified
            let Some(watch_id) = opt_watch_id else {
                apibail_internal!("Must specify a watch id in order to cancel it");
            };
            // Ensure watch node is specified
            let Some(watch_node) = opt_watch_node else {
                apibail_internal!("Must specify a watch node in order to cancel it");
            };
            return self
                .outbound_watch_value_cancel(
                    rpc_processor,
                    key,
                    subkeys,
                    safety_selection,
                    opt_watcher,
                    watch_id,
                    watch_node,
                )
                .await;
        }

        // if the watch id and watch node are specified, then we're trying to change an existing watch
        // first try to do that, then fall back to fanout for a new watch id
        if let Some(watch_id) = opt_watch_id {
            let Some(watch_node) = opt_watch_node else {
                apibail_internal!("Must specify a watch node in order to change it");
            };
            if let Some(res) = self
                .outbound_watch_value_change(
                    rpc_processor.clone(),
                    key,
                    subkeys.clone(),
                    expiration,
                    count,
                    safety_selection,
                    opt_watcher,
                    watch_id,
                    watch_node,
                )
                .await?
            {
                // If a change was successful then return immediately
                return Ok(Some(res));
            }

            // Otherwise, treat this like a new watch
        }

        let routing_table = rpc_processor.routing_table();

        // Get the DHT parameters for 'WatchValue', some of which are the same for 'SetValue' operations
        let (key_count, timeout_us, set_value_count) = {
            let c = self.unlocked_inner.config.get();
            (
                c.network.dht.max_find_node_count as usize,
                TimestampDuration::from(ms_to_us(c.network.dht.set_value_timeout_ms)),
                c.network.dht.set_value_count as usize,
            )
        };

        // Get the appropriate watcher key, if anonymous use a static anonymous watch key
        // which lives for the duration of the app's runtime
        let watcher = opt_watcher.unwrap_or_else(|| {
            self.unlocked_inner
                .anonymous_watch_keys
                .get(key.kind)
                .unwrap()
                .value
        });

        // Get the nodes we know are caching this value to seed the fanout
        let init_fanout_queue = {
            let inner = self.inner.lock().await;
            inner
                .get_value_nodes(key)?
                .unwrap_or_default()
                .into_iter()
                .filter(|x| {
                    x.node_info(RoutingDomain::PublicInternet)
                        .map(|ni| ni.has_all_capabilities(&[CAP_DHT, CAP_DHT_WATCH]))
                        .unwrap_or_default()
                })
                .collect()
        };

        // Make do-watch-value answer context
        let context = Arc::new(Mutex::new(OutboundWatchValueContext {
            opt_watch_value_result: None,
        }));

        // Routine to call to generate fanout
        let call_routine = |next_node: NodeRef| {
            let rpc_processor = rpc_processor.clone();
            let context = context.clone();
            let subkeys = subkeys.clone();

            async move {
                let wva = network_result_try!(
                    rpc_processor
                        .clone()
                        .rpc_call_watch_value(
                            Destination::direct(next_node.clone()).with_safety(safety_selection),
                            key,
                            subkeys,
                            expiration,
                            count,
                            watcher,
                            None
                        )
                        .await?
                );

                // Keep answer if we got one
                // (accepted means the node could provide an answer, not that the watch is active)
                if wva.answer.accepted {
                    let mut done = false;
                    if wva.answer.expiration_ts.as_u64() > 0 {
                        // If the expiration time is greater than zero this watch is active
                        log_dht!(debug "Watch created: id={} expiration_ts={} ({})", wva.answer.watch_id, debug_ts(wva.answer.expiration_ts.as_u64()), next_node);
                        done = true;
                    } else {
                        // If the returned expiration time is zero, this watch was cancelled or rejected
                        // If we are asking to cancel then check_done will stop after the first node
                    }
                    if done {
                        let mut ctx = context.lock();
                        ctx.opt_watch_value_result = Some(OutboundWatchValueResult {
                            expiration_ts: wva.answer.expiration_ts,
                            watch_id: wva.answer.watch_id,
                            watch_node: next_node.clone(),
                            opt_value_changed_route: wva.reply_private_route,
                        });
                    }
                }

                // Return peers if we have some
                log_network_result!(debug "WatchValue fanout call returned peers {} ({})", wva.answer.peers.peer_info_list.len(), next_node);

                Ok(NetworkResult::value(wva.answer.peers))
            }.instrument(tracing::trace_span!("outbound_watch_value call routine"))
        };

        // Routine to call to check if we're done at each step
        let check_done = |_closest_nodes: &[NodeRef]| {
            // If a watch has succeeded, return done
            let ctx = context.lock();
            if ctx.opt_watch_value_result.is_some() {
                return Some(());
            }
            None
        };

        // Call the fanout
        // Use a fixed fanout concurrency of 1 because we only want one watch
        // Use a longer timeout (timeout_us * set_value_count) because we may need to try multiple nodes
        // and each one might take timeout_us time.
        let fanout_call = FanoutCall::new(
            routing_table.clone(),
            key,
            key_count,
            1,
            TimestampDuration::new(timeout_us.as_u64() * (set_value_count as u64)),
            capability_fanout_node_info_filter(vec![CAP_DHT, CAP_DHT_WATCH]),
            call_routine,
            check_done,
        );

        match fanout_call.run(init_fanout_queue).await {
            // If we don't finish in the timeout (too much time passed without a successful watch)
            TimeoutOr::Timeout => {
                // Return the best answer we've got
                let ctx = context.lock();
                if ctx.opt_watch_value_result.is_some() {
                    log_dht!(debug "WatchValue Fanout Timeout Success");
                } else {
                    log_dht!(debug "WatchValue Fanout Timeout Failure");
                }
                Ok(ctx.opt_watch_value_result.clone())
            }
            // If we finished with done
            TimeoutOr::Value(Ok(Some(()))) => {
                // Return the best answer we've got
                let ctx = context.lock();
                if ctx.opt_watch_value_result.is_some() {
                    log_dht!(debug "WatchValue Fanout Success");
                } else {
                    log_dht!(debug "WatchValue Fanout Failure");
                }
                Ok(ctx.opt_watch_value_result.clone())
            }
            // If we ran out of nodes
            TimeoutOr::Value(Ok(None)) => {
                // Return the best answer we've got
                let ctx = context.lock();
                if ctx.opt_watch_value_result.is_some() {
                    log_dht!(debug "WatchValue Fanout Exhausted Success");
                } else {
                    log_dht!(debug "WatchValue Fanout Exhausted Failure");
                }
                Ok(ctx.opt_watch_value_result.clone())
            }
            // Failed
            TimeoutOr::Value(Err(e)) => {
                // If we finished with an error, return that
                log_dht!(debug "WatchValue Fanout Error: {}", e);
                Err(e.into())
            }
        }
    }

    /// Handle a received 'Watch Value' query
    #[allow(clippy::too_many_arguments)]
    #[instrument(level = "trace", target = "dht", skip_all)]
    pub async fn inbound_watch_value(
        &self,
        key: TypedKey,
        params: WatchParameters,
        watch_id: Option<u64>,
    ) -> VeilidAPIResult<NetworkResult<WatchResult>> {
        let mut inner = self.lock().await?;

        // Validate input
        if params.count == 0 && (watch_id.unwrap_or_default() == 0) {
            // Can't cancel a watch without a watch id
            return VeilidAPIResult::Ok(NetworkResult::invalid_message(
                "can't cancel watch without id",
            ));
        }

        // Try from local and remote record stores
        let Some(local_record_store) = inner.local_record_store.as_mut() else {
            apibail_not_initialized!();
        };
        if local_record_store.contains_record(key) {
            return local_record_store
                .watch_record(key, params, watch_id)
                .await
                .map(NetworkResult::value);
        }
        let Some(remote_record_store) = inner.remote_record_store.as_mut() else {
            apibail_not_initialized!();
        };
        if remote_record_store.contains_record(key) {
            return remote_record_store
                .watch_record(key, params, watch_id)
                .await
                .map(NetworkResult::value);
        }
        // No record found
        Ok(NetworkResult::value(WatchResult::Rejected))
    }

    /// Handle a received 'Value Changed' statement
    #[instrument(level = "trace", target = "dht", skip_all)]
    pub async fn inbound_value_changed(
        &self,
        key: TypedKey,
        subkeys: ValueSubkeyRangeSet,
        mut count: u32,
        value: Option<Arc<SignedValueData>>,
        inbound_node_id: TypedKey,
        watch_id: u64,
    ) -> VeilidAPIResult<NetworkResult<()>> {
        // Update local record store with new value
        let (is_value_seq_newer, value) = {
            let mut inner = self.lock().await?;

            // Don't process update if the record is closed
            let Some(opened_record) = inner.opened_records.get_mut(&key) else {
                return Ok(NetworkResult::value(()));
            };

            // No active watch means no callback
            let Some(mut active_watch) = opened_record.active_watch() else {
                return Ok(NetworkResult::value(()));
            };

            // If the watch id doesn't match, then don't process this
            if active_watch.id != watch_id {
                return Ok(NetworkResult::value(()));
            }

            // If the reporting node is not the same as our watch, don't process the value change
            if !active_watch
                .watch_node
                .node_ids()
                .contains(&inbound_node_id)
            {
                return Ok(NetworkResult::value(()));
            }

            if count > active_watch.count {
                // If count is greater than our requested count then this is invalid, cancel the watch
                log_stor!(debug "watch count went backward: {}: {}/{}", key, count, active_watch.count);
                // Force count to zero
                count = 0;
                opened_record.clear_active_watch();
            } else if count == 0 {
                // If count is zero, we're done, cancel the watch and the app can renew it if it wants
                log_stor!(debug "watch count finished: {}", key);
                opened_record.clear_active_watch();
            } else {
                log_stor!(debug
                    "watch count decremented: {}: {}/{}",
                    key,
                    count,
                    active_watch.count
                );
                active_watch.count = count;
                opened_record.set_active_watch(active_watch);
            }

            // Null out default value
            let value = value.filter(|value| *value.value_data() != ValueData::default());

            // Set the local value
            let mut is_value_seq_newer = false;
            if let Some(value) = &value {
                let Some(first_subkey) = subkeys.first() else {
                    apibail_internal!("should not have value without first subkey");
                };

                let last_get_result = inner
                    .handle_get_local_value(key, first_subkey, true)
                    .await?;

                let descriptor = last_get_result.opt_descriptor.unwrap();
                let schema = descriptor.schema()?;

                // Validate with schema
                if !schema.check_subkey_value_data(
                    descriptor.owner(),
                    first_subkey,
                    value.value_data(),
                ) {
                    // Validation failed, ignore this value
                    // Move to the next node
                    return Ok(NetworkResult::invalid_message(format!(
                        "Schema validation failed on subkey {}",
                        first_subkey
                    )));
                }

                // Make sure this value would actually be newer
                is_value_seq_newer = true;
                if let Some(last_value) = &last_get_result.opt_value {
                    if value.value_data().seq() <= last_value.value_data().seq() {
                        // inbound value is older than or equal to the sequence number that we have, just return the one we have
                        is_value_seq_newer = false;
                    }
                }
                if is_value_seq_newer {
                    inner
                        .handle_set_local_value(
                            key,
                            first_subkey,
                            value.clone(),
                            WatchUpdateMode::NoUpdate,
                        )
                        .await?;
                }
            }

            (is_value_seq_newer, value)
        };

        // Announce ValueChanged VeilidUpdate
        // * if the value in the update had a newer sequence number
        // * if more than a single subkeys has changed
        // * if the count was zero meaning cancelled

        let do_update = is_value_seq_newer || subkeys.len() > 1 || count == 0;
        if do_update {
            let value = if is_value_seq_newer {
                Some(value.unwrap().value_data().clone())
            } else {
                None
            };
            self.update_callback_value_change(key, subkeys, count, value)
                .await?;
        }

        Ok(NetworkResult::value(()))
    }
}
