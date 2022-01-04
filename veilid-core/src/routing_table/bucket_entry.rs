use super::*;

// Reliable pings are done with increased spacing between pings
// - Start secs is the number of seconds between the first two pings
// - Max secs is the maximum number of seconds between consecutive pings
// - Multiplier changes the number of seconds between pings over time
//   making it longer as the node becomes more reliable
const RELIABLE_PING_INTERVAL_START_SECS: u32 = 10;
const RELIABLE_PING_INTERVAL_MAX_SECS: u32 = 10 * 60;
const RELIABLE_PING_INTERVAL_MULTIPLIER: f64 = 2.0;

// Unreliable pings are done for a fixed amount of time while the
// node is given a chance to come back online before it is made dead
// If a node misses a single ping, it is marked unreliable and must
// return reliable pings for the duration of the span before being
// marked reliable again
// - Span is the number of seconds total to attempt to validate the node
// - Interval is the number of seconds between each ping
const UNRELIABLE_PING_SPAN_SECS: u32 = 60;
const UNRELIABLE_PING_INTERVAL_SECS: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BucketEntryState {
    Dead,
    Unreliable,
    Reliable,
}

#[derive(Debug, Clone)]
pub struct BucketEntry {
    pub(super) ref_count: u32,
    min_max_version: Option<(u8, u8)>,
    last_connection: Option<(ConnectionDescriptor, u64)>,
    dial_infos: Vec<DialInfo>,
    stats_accounting: StatsAccounting,
    peer_stats: PeerStats,
}

impl BucketEntry {
    pub(super) fn new() -> Self {
        let now = get_timestamp();
        Self {
            ref_count: 0,
            min_max_version: None,
            last_connection: None,
            dial_infos: Vec::new(),
            stats_accounting: StatsAccounting::new(),
            peer_stats: PeerStats {
                time_added: now,
                last_seen: None,
                ping_stats: PingStats::default(),
                latency: None,
                transfer: TransferStatsDownUp::default(),
                node_info: None,
            },
        }
    }

    pub fn update_dial_infos(&mut self, dial_infos: &[DialInfo]) {
        self.dial_infos = dial_infos.to_vec();
        self.dial_infos.sort();
    }

    pub fn first_filtered_dial_info<F>(&self, filter: F) -> Option<DialInfo>
    where
        F: Fn(&DialInfo) -> bool,
    {
        for di in &self.dial_infos {
            if filter(di) {
                return Some(di.clone());
            }
        }
        None
    }
    pub fn all_filtered_dial_infos<F>(&self, filter: F) -> Vec<DialInfo>
    where
        F: Fn(&DialInfo) -> bool,
    {
        let mut ret = Vec::new();
        for di in &self.dial_infos {
            if filter(di) {
                ret.push(di.clone());
            }
        }
        ret
    }

    pub fn dial_infos(&self) -> &[DialInfo] {
        &self.dial_infos
    }

    pub fn get_peer_info(&self, key: DHTKey, scope: PeerScope) -> PeerInfo {
        PeerInfo {
            node_id: NodeId::new(key),
            dial_infos: self.all_filtered_dial_infos(|di| di.matches_peer_scope(scope)),
        }
    }

    pub fn set_last_connection(&mut self, last_connection: ConnectionDescriptor, timestamp: u64) {
        self.last_connection = Some((last_connection, timestamp));
    }

    pub fn last_connection(&self) -> Option<ConnectionDescriptor> {
        self.last_connection.as_ref().map(|x| x.0)
    }

    pub fn set_min_max_version(&mut self, min_max_version: (u8, u8)) {
        self.min_max_version = Some(min_max_version);
    }

    pub fn min_max_version(&self) -> Option<(u8, u8)> {
        self.min_max_version
    }

    pub fn state(&self, cur_ts: u64) -> BucketEntryState {
        if self.check_reliable(cur_ts) {
            BucketEntryState::Reliable
        } else if self.check_dead(cur_ts) {
            BucketEntryState::Dead
        } else {
            BucketEntryState::Unreliable
        }
    }

    pub fn peer_stats(&self) -> &PeerStats {
        &self.peer_stats
    }

    pub fn update_node_info(&mut self, node_info: NodeInfo) {
        self.peer_stats.node_info = Some(node_info);
    }

    ///// stats methods
    // called every ROLLING_TRANSFERS_INTERVAL_SECS seconds
    pub(super) fn roll_transfers(&mut self, last_ts: u64, cur_ts: u64) {
        self.stats_accounting
            .roll_transfers(last_ts, cur_ts, &mut self.peer_stats.transfer);
    }

    // Called for every round trip packet we receive
    fn record_latency(&mut self, latency: u64) {
        self.peer_stats.latency = Some(self.stats_accounting.record_latency(latency));
    }

    ///// state machine handling
    pub(super) fn check_reliable(&self, cur_ts: u64) -> bool {
        // if we have had consecutive ping replies for longer that UNRELIABLE_PING_SPAN_SECS
        match self.peer_stats.ping_stats.first_consecutive_pong_time {
            None => false,
            Some(ts) => {
                cur_ts.saturating_sub(ts) >= (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64)
            }
        }
    }
    pub(super) fn check_dead(&self, cur_ts: u64) -> bool {
        // if we have not heard from the node at all for the duration of the unreliable ping span
        // a node is not dead if we haven't heard from it yet
        match self.peer_stats.last_seen {
            None => false,
            Some(ts) => {
                cur_ts.saturating_sub(ts) >= (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64)
            }
        }
    }

    pub(super) fn needs_ping(&self, cur_ts: u64) -> bool {
        // if we need a ping right now to validate us
        match self.state(cur_ts) {
            BucketEntryState::Reliable => {
                // If we are in a reliable state, we need a ping on an exponential scale
                match self.peer_stats.ping_stats.last_pinged {
                    None => true,
                    Some(last_pinged) => {
                        let first_consecutive_pong_time = self
                            .peer_stats
                            .ping_stats
                            .first_consecutive_pong_time
                            .unwrap();
                        let start_of_reliable_time = first_consecutive_pong_time
                            + ((UNRELIABLE_PING_SPAN_SECS - UNRELIABLE_PING_INTERVAL_SECS) as u64
                                * 1000000u64);
                        let reliable_cur = cur_ts.saturating_sub(start_of_reliable_time);
                        let reliable_last = last_pinged.saturating_sub(start_of_reliable_time);

                        retry_falloff_log(
                            reliable_last,
                            reliable_cur,
                            RELIABLE_PING_INTERVAL_START_SECS as u64 * 1000000u64,
                            RELIABLE_PING_INTERVAL_MAX_SECS as u64 * 1000000u64,
                            RELIABLE_PING_INTERVAL_MULTIPLIER,
                        )
                    }
                }
            }
            BucketEntryState::Unreliable => {
                // If we are in an unreliable state, we need a ping every UNRELIABLE_PING_INTERVAL_SECS seconds
                match self.peer_stats.ping_stats.last_pinged {
                    None => true,
                    Some(last_pinged) => {
                        cur_ts.saturating_sub(last_pinged)
                            >= (UNRELIABLE_PING_INTERVAL_SECS as u64 * 1000000u64)
                    }
                }
            }
            BucketEntryState::Dead => false,
        }
    }

    pub(super) fn touch_last_seen(&mut self, ts: u64) {
        // If we've heard from the node at all, we can always restart our lost ping count
        self.peer_stats.ping_stats.recent_lost_pings = 0;
        // Mark the node as seen
        self.peer_stats.last_seen = Some(ts);
    }

    pub(super) fn state_debug_info(&self, cur_ts: u64) -> String {
        let last_pinged = if let Some(last_pinged) = self.peer_stats.ping_stats.last_pinged {
            format!(
                "{}s ago",
                timestamp_to_secs(cur_ts.saturating_sub(last_pinged))
            )
        } else {
            "never".to_owned()
        };
        let first_consecutive_pong_time = if let Some(first_consecutive_pong_time) =
            self.peer_stats.ping_stats.first_consecutive_pong_time
        {
            format!(
                "{}s ago",
                timestamp_to_secs(cur_ts.saturating_sub(first_consecutive_pong_time))
            )
        } else {
            "never".to_owned()
        };
        let last_seen = if let Some(last_seen) = self.peer_stats.last_seen {
            format!(
                "{}s ago",
                timestamp_to_secs(cur_ts.saturating_sub(last_seen))
            )
        } else {
            "never".to_owned()
        };

        format!(
            "state: {:?}, first_consecutive_pong_time: {}, last_pinged: {}, last_seen: {}",
            self.state(cur_ts),
            first_consecutive_pong_time,
            last_pinged,
            last_seen
        )
    }

    ////////////////////////////////////////////////////////////////
    /// Called when rpc processor things happen

    pub(super) fn ping_sent(&mut self, ts: u64, bytes: u64) {
        self.peer_stats.ping_stats.total_sent += 1;
        self.stats_accounting.add_up(bytes);
        self.peer_stats.ping_stats.in_flight += 1;
        self.peer_stats.ping_stats.last_pinged = Some(ts);
        // if we haven't heard from this node yet and it's our first attempt at contacting it
        // then we set the last_seen time
        if self.peer_stats.last_seen.is_none() {
            self.peer_stats.last_seen = Some(ts);
        }
    }
    pub(super) fn ping_rcvd(&mut self, ts: u64, bytes: u64) {
        self.stats_accounting.add_down(bytes);
        self.touch_last_seen(ts);
    }
    pub(super) fn pong_sent(&mut self, _ts: u64, bytes: u64) {
        self.stats_accounting.add_up(bytes);
    }
    pub(super) fn pong_rcvd(&mut self, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.stats_accounting.add_down(bytes);
        self.peer_stats.ping_stats.in_flight -= 1;
        self.peer_stats.ping_stats.total_returned += 1;
        self.peer_stats.ping_stats.consecutive_pongs += 1;
        if self
            .peer_stats
            .ping_stats
            .first_consecutive_pong_time
            .is_none()
        {
            self.peer_stats.ping_stats.first_consecutive_pong_time = Some(recv_ts);
        }
        self.record_latency(recv_ts - send_ts);
        self.touch_last_seen(recv_ts);
    }
    pub(super) fn ping_lost(&mut self, _ts: u64) {
        self.peer_stats.ping_stats.in_flight -= 1;
        self.peer_stats.ping_stats.recent_lost_pings += 1;
        self.peer_stats.ping_stats.consecutive_pongs = 0;
        self.peer_stats.ping_stats.first_consecutive_pong_time = None;
    }
    pub(super) fn question_sent(&mut self, ts: u64, bytes: u64) {
        self.stats_accounting.add_up(bytes);
        // if we haven't heard from this node yet and it's our first attempt at contacting it
        // then we set the last_seen time
        if self.peer_stats.last_seen.is_none() {
            self.peer_stats.last_seen = Some(ts);
        }
    }
    pub(super) fn question_rcvd(&mut self, ts: u64, bytes: u64) {
        self.stats_accounting.add_down(bytes);
        self.touch_last_seen(ts);
    }
    pub(super) fn answer_sent(&mut self, _ts: u64, bytes: u64) {
        self.stats_accounting.add_up(bytes);
    }
    pub(super) fn answer_rcvd(&mut self, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.stats_accounting.add_down(bytes);
        self.record_latency(recv_ts - send_ts);
        self.touch_last_seen(recv_ts);
    }
    pub(super) fn question_lost(&mut self, _ts: u64) {
        self.peer_stats.ping_stats.consecutive_pongs = 0;
        self.peer_stats.ping_stats.first_consecutive_pong_time = None;
    }
}

impl Drop for BucketEntry {
    fn drop(&mut self) {
        assert_eq!(self.ref_count, 0);
    }
}
