use super::*;

use crate::dht::*;
use crate::xx::*;
use crate::*;

impl RoutingTable {
    // Compute transfer statistics to determine how 'fast' a node is
    #[instrument(level = "trace", skip(self), err)]
    pub(super) async fn rolling_transfers_task_routine(
        self,
        stop_token: StopToken,
        last_ts: u64,
        cur_ts: u64,
    ) -> Result<(), String> {
        // log_rtab!("--- rolling_transfers task");
        let mut inner = self.inner.write();
        let inner = &mut *inner;

        // Roll our own node's transfers
        inner.self_transfer_stats_accounting.roll_transfers(
            last_ts,
            cur_ts,
            &mut inner.self_transfer_stats,
        );

        // Roll all bucket entry transfers
        for b in &mut inner.buckets {
            b.roll_transfers(last_ts, cur_ts);
        }
        Ok(())
    }

    // Bootstrap lookup process
    #[instrument(level = "trace", skip(self), ret, err)]
    pub(super) async fn resolve_bootstrap(
        &self,
        bootstrap: Vec<String>,
    ) -> Result<BootstrapRecordMap, String> {
        // Resolve from bootstrap root to bootstrap hostnames
        let mut bsnames = Vec::<String>::new();
        for bh in bootstrap {
            // Get TXT record for bootstrap (bootstrap.veilid.net, or similar)
            let records = intf::txt_lookup(&bh).await?;
            for record in records {
                // Split the bootstrap name record by commas
                for rec in record.split(',') {
                    let rec = rec.trim();
                    // If the name specified is fully qualified, go with it
                    let bsname = if rec.ends_with('.') {
                        rec.to_string()
                    }
                    // If the name is not fully qualified, prepend it to the bootstrap name
                    else {
                        format!("{}.{}", rec, bh)
                    };

                    // Add to the list of bootstrap name to look up
                    bsnames.push(bsname);
                }
            }
        }

        // Get bootstrap nodes from hostnames concurrently
        let mut unord = FuturesUnordered::new();
        for bsname in bsnames {
            unord.push(async move {
                // look up boostrap node txt records
                let bsnirecords = match intf::txt_lookup(&bsname).await {
                    Err(e) => {
                        warn!("bootstrap node txt lookup failed for {}: {}", bsname, e);
                        return None;
                    }
                    Ok(v) => v,
                };
                // for each record resolve into key/bootstraprecord pairs
                let mut bootstrap_records: Vec<(DHTKey, BootstrapRecord)> = Vec::new();
                for bsnirecord in bsnirecords {
                    // Bootstrap TXT Record Format Version 0:
                    // txt_version,min_version,max_version,nodeid,hostname,dialinfoshort*
                    //
                    // Split bootstrap node record by commas. Example:
                    // 0,0,0,7lxDEabK_qgjbe38RtBa3IZLrud84P6NhGP-pRTZzdQ,bootstrap-dev-alpha.veilid.net,T5150,U5150,W5150/ws
                    let records: Vec<String> = bsnirecord
                        .trim()
                        .split(',')
                        .map(|x| x.trim().to_owned())
                        .collect();
                    if records.len() < 6 {
                        warn!("invalid number of fields in bootstrap txt record");
                        continue;
                    }

                    // Bootstrap TXT record version
                    let txt_version: u8 = match records[0].parse::<u8>() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "invalid txt_version specified in bootstrap node txt record: {}",
                                e
                            );
                            continue;
                        }
                    };
                    if txt_version != BOOTSTRAP_TXT_VERSION {
                        warn!("unsupported bootstrap txt record version");
                        continue;
                    }

                    // Min/Max wire protocol version
                    let min_version: u8 = match records[1].parse::<u8>() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "invalid min_version specified in bootstrap node txt record: {}",
                                e
                            );
                            continue;
                        }
                    };
                    let max_version: u8 = match records[2].parse::<u8>() {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "invalid max_version specified in bootstrap node txt record: {}",
                                e
                            );
                            continue;
                        }
                    };

                    // Node Id
                    let node_id_str = &records[3];
                    let node_id_key = match DHTKey::try_decode(node_id_str) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "Invalid node id in bootstrap node record {}: {}",
                                node_id_str, e
                            );
                            continue;
                        }
                    };

                    // Hostname
                    let hostname_str = &records[4];

                    // If this is our own node id, then we skip it for bootstrap, in case we are a bootstrap node
                    if self.node_id() == node_id_key {
                        continue;
                    }

                    // Resolve each record and store in node dial infos list
                    let mut bootstrap_record = BootstrapRecord {
                        min_version,
                        max_version,
                        dial_info_details: Vec::new(),
                    };
                    for rec in &records[5..] {
                        let rec = rec.trim();
                        let dial_infos = match DialInfo::try_vec_from_short(rec, hostname_str) {
                            Ok(dis) => dis,
                            Err(e) => {
                                warn!("Couldn't resolve bootstrap node dial info {}: {}", rec, e);
                                continue;
                            }
                        };

                        for di in dial_infos {
                            bootstrap_record.dial_info_details.push(DialInfoDetail {
                                dial_info: di,
                                class: DialInfoClass::Direct,
                            });
                        }
                    }
                    bootstrap_records.push((node_id_key, bootstrap_record));
                }
                Some(bootstrap_records)
            });
        }

        let mut bsmap = BootstrapRecordMap::new();
        while let Some(bootstrap_records) = unord.next().await {
            if let Some(bootstrap_records) = bootstrap_records {
                for (bskey, mut bsrec) in bootstrap_records {
                    let rec = bsmap.entry(bskey).or_insert_with(|| BootstrapRecord {
                        min_version: bsrec.min_version,
                        max_version: bsrec.max_version,
                        dial_info_details: Vec::new(),
                    });
                    rec.dial_info_details.append(&mut bsrec.dial_info_details);
                }
            }
        }

        Ok(bsmap)
    }

    #[instrument(level = "trace", skip(self), err)]
    pub(super) async fn bootstrap_task_routine(self, stop_token: StopToken) -> Result<(), String> {
        let (bootstrap, bootstrap_nodes) = {
            let c = self.config.get();
            (
                c.network.bootstrap.clone(),
                c.network.bootstrap_nodes.clone(),
            )
        };

        log_rtab!(debug "--- bootstrap_task");

        // If we aren't specifying a bootstrap node list explicitly, then pull from the bootstrap server(s)

        let bsmap: BootstrapRecordMap = if !bootstrap_nodes.is_empty() {
            let mut bsmap = BootstrapRecordMap::new();
            let mut bootstrap_node_dial_infos = Vec::new();
            for b in bootstrap_nodes {
                let ndis = NodeDialInfo::from_str(b.as_str())
                    .map_err(map_to_string)
                    .map_err(logthru_rtab!(
                        "Invalid node dial info in bootstrap entry: {}",
                        b
                    ))?;
                bootstrap_node_dial_infos.push(ndis);
            }
            for ndi in bootstrap_node_dial_infos {
                let node_id = ndi.node_id.key;
                bsmap
                    .entry(node_id)
                    .or_insert_with(|| BootstrapRecord {
                        min_version: MIN_VERSION,
                        max_version: MAX_VERSION,
                        dial_info_details: Vec::new(),
                    })
                    .dial_info_details
                    .push(DialInfoDetail {
                        dial_info: ndi.dial_info,
                        class: DialInfoClass::Direct, // Bootstraps are always directly reachable
                    });
            }
            bsmap
        } else {
            // Resolve bootstrap servers and recurse their TXT entries
            self.resolve_bootstrap(bootstrap).await?
        };

        // Map all bootstrap entries to a single key with multiple dialinfo

        // Run all bootstrap operations concurrently
        let mut unord = FuturesUnordered::new();
        for (k, mut v) in bsmap {
            // Sort dial info so we get the preferred order correct
            v.dial_info_details.sort();

            log_rtab!("--- bootstrapping {} with {:?}", k.encode(), &v);

            // Make invalid signed node info (no signature)
            let nr = self
                .register_node_with_signed_node_info(
                    k,
                    SignedNodeInfo::with_no_signature(NodeInfo {
                        network_class: NetworkClass::InboundCapable, // Bootstraps are always inbound capable
                        outbound_protocols: ProtocolSet::empty(), // Bootstraps do not participate in relaying and will not make outbound requests
                        min_version: v.min_version, // Minimum protocol version specified in txt record
                        max_version: v.max_version, // Maximum protocol version specified in txt record
                        dial_info_detail_list: v.dial_info_details, // Dial info is as specified in the bootstrap list
                        relay_peer_info: None, // Bootstraps never require a relay themselves
                    }),
                )
                .map_err(logthru_rtab!(error "Couldn't add bootstrap node: {}", k))?;

            // Add this our futures to process in parallel
            let this = self.clone();
            unord.push(async move {
                // Need VALID signed peer info, so ask bootstrap to find_node of itself
                // which will ensure it has the bootstrap's signed peer info as part of the response
                let _ = this.find_target(nr.clone()).await;

                // Ensure we got the signed peer info
                if !nr.operate(|e| e.has_valid_signed_node_info()) {
                    log_rtab!(warn
                        "bootstrap at {:?} did not return valid signed node info",
                        nr
                    );
                    // If this node info is invalid, it will time out after being unpingable
                } else {
                    // otherwise this bootstrap is valid, lets ask it to find ourselves now
                    this.reverse_find_node(nr, true).await
                }
            });
        }

        // Wait for all bootstrap operations to complete before we complete the singlefuture
        while unord.next().await.is_some() {}
        Ok(())
    }

    // Ping each node in the routing table if they need to be pinged
    // to determine their reliability
    #[instrument(level = "trace", skip(self), err)]
    pub(super) async fn ping_validator_task_routine(
        self,
        stop_token: StopToken,
        _last_ts: u64,
        cur_ts: u64,
    ) -> Result<(), String> {
        let rpc = self.rpc_processor();
        let netman = self.network_manager();
        let relay_node_id = netman.relay_node().map(|nr| nr.node_id());

        let mut unord = FuturesUnordered::new();
        {
            let inner = self.inner.read();

            Self::with_entries_unlocked(&*inner, cur_ts, BucketEntryState::Unreliable, |k, v| {
                if v.with(|e| e.needs_ping(&k, cur_ts, relay_node_id)) {
                    let nr = NodeRef::new(self.clone(), k, v, None);
                    unord.push(MustJoinHandle::new(intf::spawn_local(
                        rpc.clone().rpc_call_status(nr),
                    )));
                }
                Option::<()>::None
            });
        }

        // Wait for futures to complete
        while unord.next().await.is_some() {}

        Ok(())
    }

    // Ask our remaining peers to give us more peers before we go
    // back to the bootstrap servers to keep us from bothering them too much
    #[instrument(level = "trace", skip(self), err)]
    pub(super) async fn peer_minimum_refresh_task_routine(
        self,
        stop_token: StopToken,
    ) -> Result<(), String> {
        // get list of all peers we know about, even the unreliable ones, and ask them to find nodes close to our node too
        let noderefs = {
            let inner = self.inner.read();
            let mut noderefs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
            let cur_ts = intf::get_timestamp();
            Self::with_entries_unlocked(&*inner, cur_ts, BucketEntryState::Unreliable, |k, v| {
                noderefs.push(NodeRef::new(self.clone(), k, v, None));
                Option::<()>::None
            });
            noderefs
        };

        // do peer minimum search concurrently
        let mut unord = FuturesUnordered::new();
        for nr in noderefs {
            log_rtab!("--- peer minimum search with {:?}", nr);
            unord.push(self.reverse_find_node(nr, false));
        }
        while unord.next().await.is_some() {}

        Ok(())
    }

    // Kick the queued buckets in the routing table to free dead nodes if necessary
    // Attempts to keep the size of the routing table down to the bucket depth
    #[instrument(level = "trace", skip(self), err)]
    pub(super) async fn kick_buckets_task_routine(
        self,
        _stop_token: StopToken,
        _last_ts: u64,
        cur_ts: u64,
    ) -> Result<(), String> {
        let mut inner = self.inner.write();
        let kick_queue: Vec<usize> = inner.kick_queue.iter().map(|v| *v).collect();
        inner.kick_queue.clear();
        for idx in kick_queue {
            Self::kick_bucket(&mut *inner, idx)
        }
        Ok(())
    }
}
