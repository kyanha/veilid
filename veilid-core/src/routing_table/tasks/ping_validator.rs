use super::*;

/// Keepalive pings are done occasionally to ensure holepunched public dialinfo
/// remains valid, as well as to make sure we remain in any relay node's routing table
const RELAY_KEEPALIVE_PING_INTERVAL_SECS: u32 = 10;

/// Keepalive pings are done for active watch nodes to make sure they are still there
const ACTIVE_WATCH_KEEPALIVE_PING_INTERVAL_SECS: u32 = 10;

/// Ping queue processing depth
const MAX_PARALLEL_PINGS: usize = 16;

use futures_util::stream::{FuturesUnordered, StreamExt};
use futures_util::FutureExt;
use stop_token::future::FutureExt as StopFutureExt;

type PingValidatorFuture = SendPinBoxFuture<Result<(), RPCError>>;

impl RoutingTable {
    // Ping the relay to keep it alive, over every protocol it is relaying for us
    #[instrument(level = "trace", skip(self, futurequeue), err)]
    async fn relay_keepalive_public_internet(
        &self,
        cur_ts: Timestamp,
        futurequeue: &mut VecDeque<PingValidatorFuture>,
    ) -> EyreResult<()> {
        // Get the PublicInternet relay if we are using one
        let Some(relay_nr) = self.relay_node(RoutingDomain::PublicInternet) else {
            return Ok(());
        };

        let rpc = self.rpc_processor();
        // Get our publicinternet dial info
        let dids = self.all_filtered_dial_info_details(
            RoutingDomain::PublicInternet.into(),
            &DialInfoFilter::all(),
        );

        let opt_relay_keepalive_ts = self.relay_node_last_keepalive(RoutingDomain::PublicInternet);
        let relay_needs_keepalive = opt_relay_keepalive_ts
            .map(|kts| {
                cur_ts.saturating_sub(kts).as_u64()
                    >= (RELAY_KEEPALIVE_PING_INTERVAL_SECS as u64 * 1_000_000u64)
            })
            .unwrap_or(true);

        if !relay_needs_keepalive {
            return Ok(());
        }
        // Say we're doing this keepalive now
        self.edit_public_internet_routing_domain()
            .set_relay_node_keepalive(Some(cur_ts))
            .commit(false)
            .await;

        // We need to keep-alive at one connection per ordering for relays
        // but also one per NAT mapping that we need to keep open for our inbound dial info
        let mut got_unordered = false;
        let mut got_ordered = false;

        // Look up any NAT mappings we may need to try to preserve with keepalives
        let mut mapped_port_info = self.get_low_level_port_info();

        // Relay nodes get pinged over all protocols we have inbound dialinfo for
        // This is so we can preserve the inbound NAT mappings at our router
        let mut relay_noderefs = vec![];
        for did in &dids {
            // Can skip the ones that are direct, those are not mapped or natted
            // because we can have both direct and natted dialinfo on the same
            // node, for example ipv4 can be natted, while ipv6 is direct
            if did.class == DialInfoClass::Direct {
                continue;
            }
            // Do we need to do this ping?
            // Check if we have already pinged over this low-level-protocol/address-type/port combo
            // We want to ensure we do the bare minimum required here
            let pt = did.dial_info.protocol_type();
            let at = did.dial_info.address_type();
            let needs_ping_for_protocol =
                if let Some((llpt, port)) = mapped_port_info.protocol_to_port.get(&(pt, at)) {
                    mapped_port_info
                        .low_level_protocol_ports
                        .remove(&(*llpt, at, *port))
                } else {
                    false
                };
            if needs_ping_for_protocol {
                if pt.is_ordered() {
                    got_ordered = true;
                } else {
                    got_unordered = true;
                }
                let dif = did.dial_info.make_filter();

                relay_noderefs
                    .push(relay_nr.filtered_clone(NodeRefFilter::new().with_dial_info_filter(dif)));
            }
        }
        // Add noderef filters for ordered or unordered sequencing if we havent already seen those
        if !got_ordered {
            relay_noderefs.push(relay_nr.sequencing_clone(Sequencing::EnsureOrdered));
        }
        if !got_unordered {
            relay_noderefs.push(relay_nr);
        }

        for relay_nr_filtered in relay_noderefs {
            let rpc = rpc.clone();

            log_rtab!("--> Keepalive ping to {:?}", relay_nr_filtered);

            futurequeue.push_back(
                async move {
                    let _ = rpc
                        .rpc_call_status(Destination::direct(relay_nr_filtered))
                        .await?;
                    Ok(())
                }
                .boxed(),
            );
        }
        Ok(())
    }

    // Ping the active watch nodes to ensure they are still there
    #[instrument(level = "trace", skip(self, futurequeue), err)]
    async fn active_watches_keepalive_public_internet(
        &self,
        cur_ts: Timestamp,
        futurequeue: &mut VecDeque<PingValidatorFuture>,
    ) -> EyreResult<()> {
        let rpc = self.rpc_processor();

        let watches_need_keepalive = {
            let mut inner = self.inner.write();
            let need = inner
                .opt_active_watch_keepalive_ts
                .map(|kts| {
                    cur_ts.saturating_sub(kts).as_u64()
                        >= (ACTIVE_WATCH_KEEPALIVE_PING_INTERVAL_SECS as u64 * 1_000_000u64)
                })
                .unwrap_or(true);
            if need {
                inner.opt_active_watch_keepalive_ts = Some(cur_ts);
            }
            need
        };

        if !watches_need_keepalive {
            return Ok(());
        }

        // Get all the active watches from the storage manager
        let storage_manager = self.unlocked_inner.network_manager.storage_manager();
        let watch_node_refs = storage_manager.get_active_watch_nodes().await;

        for watch_nr in watch_node_refs {
            let rpc = rpc.clone();

            log_rtab!("--> Watch ping to {:?}", watch_nr);

            futurequeue.push_back(
                async move {
                    let _ = rpc
                        .rpc_call_status(Destination::direct(watch_nr.default_filtered()))
                        .await?;
                    Ok(())
                }
                .boxed(),
            );
        }
        Ok(())
    }

    // Ping each node in the routing table if they need to be pinged
    // to determine their reliability
    #[instrument(level = "trace", skip(self, futurequeue), err)]
    async fn ping_validator_public_internet(
        &self,
        cur_ts: Timestamp,
        futurequeue: &mut VecDeque<PingValidatorFuture>,
    ) -> EyreResult<()> {
        let rpc = self.rpc_processor();

        // Get all nodes needing pings in the PublicInternet routing domain
        let node_refs = self.get_nodes_needing_ping(RoutingDomain::PublicInternet, cur_ts);

        // If we have a relay, let's ping for NAT keepalives and check for address changes
        self.relay_keepalive_public_internet(cur_ts, futurequeue)
            .await?;

        // Check active watch keepalives
        self.active_watches_keepalive_public_internet(cur_ts, futurequeue)
            .await?;

        // Just do a single ping with the best protocol for all the other nodes to check for liveness
        for nr in node_refs {
            let rpc = rpc.clone();
            log_rtab!("--> Validator ping to {:?}", nr);
            futurequeue.push_back(
                async move {
                    let _ = rpc.rpc_call_status(Destination::direct(nr)).await?;
                    Ok(())
                }
                .boxed(),
            );
        }

        Ok(())
    }

    // Ping each node in the LocalNetwork routing domain if they
    // need to be pinged to determine their reliability
    #[instrument(level = "trace", skip(self, futurequeue), err)]
    async fn ping_validator_local_network(
        &self,
        cur_ts: Timestamp,
        futurequeue: &mut VecDeque<PingValidatorFuture>,
    ) -> EyreResult<()> {
        let rpc = self.rpc_processor();

        // Get all nodes needing pings in the LocalNetwork routing domain
        let node_refs = self.get_nodes_needing_ping(RoutingDomain::LocalNetwork, cur_ts);

        // For all nodes needing pings, figure out how many and over what protocols
        for nr in node_refs {
            let rpc = rpc.clone();

            // Just do a single ping with the best protocol for all the nodes
            futurequeue.push_back(
                async move {
                    let _ = rpc.rpc_call_status(Destination::direct(nr)).await?;
                    Ok(())
                }
                .boxed(),
            );
        }

        Ok(())
    }

    // Ping each node in the routing table if they need to be pinged
    // to determine their reliability
    #[instrument(level = "trace", skip(self), err)]
    pub(crate) async fn ping_validator_task_routine(
        self,
        stop_token: StopToken,
        _last_ts: Timestamp,
        cur_ts: Timestamp,
    ) -> EyreResult<()> {
        let mut futurequeue: VecDeque<PingValidatorFuture> = VecDeque::new();

        // PublicInternet
        self.ping_validator_public_internet(cur_ts, &mut futurequeue)
            .await?;

        // LocalNetwork
        self.ping_validator_local_network(cur_ts, &mut futurequeue)
            .await?;

        // Wait for ping futures to complete in parallel
        let mut unord = FuturesUnordered::new();

        while !unord.is_empty() || !futurequeue.is_empty() {
            log_rtab!(
                "Ping validation queue: {} remaining, {} in progress",
                futurequeue.len(),
                unord.len()
            );

            // Process one unordered futures if we have some
            match unord
                .next()
                .timeout_at(stop_token.clone())
                .in_current_span()
                .await
            {
                Ok(Some(res)) => {
                    // Some ping completed
                    match res {
                        Ok(()) => {}
                        Err(e) => {
                            log_rtab!(error "Error performing status ping: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // We're empty
                }
                Err(_) => {
                    // Timeout means we drop the rest because we were asked to stop
                    break;
                }
            }

            // Fill unord up to max parallelism
            while unord.len() < MAX_PARALLEL_PINGS {
                let Some(fq) = futurequeue.pop_front() else {
                    break;
                };
                unord.push(fq);
            }
        }

        Ok(())
    }
}
