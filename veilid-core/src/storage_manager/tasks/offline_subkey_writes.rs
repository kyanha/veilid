use super::*;
use futures_util::*;

impl StorageManager {
    // Best-effort write subkeys to the network that were written offline
    #[instrument(level = "trace", skip(self), err)]
    pub(crate) async fn offline_subkey_writes_task_routine(
        self,
        stop_token: StopToken,
        _last_ts: Timestamp,
        _cur_ts: Timestamp,
    ) -> EyreResult<()> {
        let (mut offline_subkey_writes, opt_update_callback) = {
            let mut inner = self.lock().await?;
            let out = (
                inner.offline_subkey_writes.clone(),
                inner.update_callback.clone(),
            );
            inner.offline_subkey_writes.clear();
            out
        };

        let mut fanout_results = vec![];

        for (key, osw) in offline_subkey_writes.iter_mut() {
            if poll!(stop_token.clone()).is_ready() {
                log_stor!(debug "Offline subkey writes cancelled.");
                break;
            }
            let Some(rpc_processor) = self.online_writes_ready().await? else {
                log_stor!(debug "Offline subkey writes stopped for network.");
                break;
            };

            let mut written_subkeys = ValueSubkeyRangeSet::new();
            for subkey in osw.subkeys.iter() {
                let get_result = {
                    let mut inner = self.lock().await?;
                    inner.handle_get_local_value(*key, subkey, true).await
                };
                let Ok(get_result) = get_result else {
                    log_stor!(debug "Offline subkey write had no subkey result: {}:{}", key, subkey);
                    continue;
                };
                let Some(value) = get_result.opt_value else {
                    log_stor!(debug "Offline subkey write had no subkey value: {}:{}", key, subkey);
                    continue;
                };
                let Some(descriptor) = get_result.opt_descriptor else {
                    log_stor!(debug "Offline subkey write had no descriptor: {}:{}", key, subkey);
                    continue;
                };
                log_stor!(debug "Offline subkey write: {}:{} len={}", key, subkey, value.value_data().data().len());
                let osvres = self
                    .outbound_set_value(
                        rpc_processor.clone(),
                        *key,
                        subkey,
                        osw.safety_selection,
                        value,
                        descriptor,
                    )
                    .await;
                match osvres {
                    Ok(osv) => {
                        match osv.fanout_result.kind {
                            FanoutResultKind::Timeout => {
                                log_stor!(debug "timed out writing offline subkey: {}:{}", key, subkey);
                            }
                            FanoutResultKind::Finished | FanoutResultKind::Exhausted => {
                                if let Some(update_callback) = opt_update_callback.clone() {
                                    // Send valuechange with dead count and no subkeys
                                    update_callback(VeilidUpdate::ValueChange(Box::new(
                                        VeilidValueChange {
                                            key: *key,
                                            subkeys: ValueSubkeyRangeSet::single(subkey),
                                            count: u32::MAX,
                                            value: Some(osv.signed_value_data.value_data().clone()),
                                        },
                                    )));
                                }
                                written_subkeys.insert(subkey);
                            }
                        };

                        fanout_results.push((subkey, osv.fanout_result));
                    }
                    Err(e) => {
                        log_stor!(debug "failed to write offline subkey: {}:{} {}", key, subkey, e);
                    }
                }
            }

            osw.subkeys = osw.subkeys.difference(&written_subkeys);

            // Keep the list of nodes that returned a value for later reference
            {
                let mut inner = self.lock().await?;
                inner.process_fanout_results(
                    *key,
                    fanout_results.iter().map(|x| (x.0, &x.1)),
                    true,
                );
            }
        }

        // Add any subkeys back in that were not successfully written
        let mut inner = self.lock().await?;
        for (key, osw) in offline_subkey_writes {
            if !osw.subkeys.is_empty() {
                inner
                    .offline_subkey_writes
                    .entry(key)
                    .and_modify(|x| {
                        x.subkeys = x.subkeys.union(&osw.subkeys);
                    })
                    .or_insert(osw);
            }
        }

        Ok(())
    }
}
