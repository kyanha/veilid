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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BucketEntryState {
    Reliable,
    Unreliable,
    Dead,
}

#[derive(Debug, Clone)]
pub struct BucketEntry {
    pub(super) ref_count: u32,
    min_max_version: Option<(u8, u8)>,
    last_connection: Option<(ConnectionDescriptor, u64)>,
    dial_info_entries: VecDeque<DialInfoEntry>,
    stats_accounting: StatsAccounting,
    peer_stats: PeerStats,
}

impl BucketEntry {
    pub(super) fn new() -> Self {
        Self {
            ref_count: 0,
            min_max_version: None,
            last_connection: None,
            dial_info_entries: VecDeque::new(),
            stats_accounting: StatsAccounting::new(),
            peer_stats: PeerStats {
                time_added: get_timestamp(),
                last_seen: None,
                ping_stats: PingStats::default(),
                latency: None,
                transfer: TransferStatsDownUp::default(),
                node_info: None,
            },
        }
    }

    pub fn add_dial_info(&mut self, dial_info: DialInfo) -> Result<(), String> {
        let mut idx: Option<usize> = None;
        for i in 0..self.dial_info_entries.len() {
            if self.dial_info_entries[i].dial_info() == &dial_info {
                idx = Some(i);
                break;
            }
        }
        match idx {
            None => {
                self.dial_info_entries
                    .push_front(DialInfoEntry::try_new(dial_info)?);
            }
            Some(idx) => {
                let die = self.dial_info_entries.remove(idx).unwrap();
                self.dial_info_entries.push_front(die);
            }
        }
        Ok(())
    }

    pub fn best_dial_info(&self) -> Option<DialInfo> {
        self.dial_info_entries
            .front()
            .map(|die| die.dial_info().clone())
    }

    pub fn filtered_dial_info<F>(&self, filter: F) -> Option<DialInfo>
    where
        F: Fn(&DialInfoEntry) -> bool,
    {
        for die in &self.dial_info_entries {
            if filter(die) {
                return Some(die.dial_info().clone());
            }
        }
        None
    }

    pub fn dial_info_entries_as_ref(&self) -> &VecDeque<DialInfoEntry> {
        &self.dial_info_entries
    }

    pub fn dial_info(&self) -> Vec<DialInfo> {
        self.dial_info_entries
            .iter()
            .map(|e| e.dial_info().clone())
            .collect()
    }

    pub fn global_dial_info(&self) -> Vec<DialInfo> {
        self.dial_info_entries
            .iter()
            .filter_map(|e| {
                if e.is_public() {
                    Some(e.dial_info().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn global_dial_info_for_protocol(&self, protocol_type: ProtocolType) -> Vec<DialInfo> {
        self.dial_info_entries
            .iter()
            .filter_map(|e| {
                if e.dial_info().protocol_type() != protocol_type {
                    None
                } else if e.is_public() {
                    Some(e.dial_info().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn local_dial_info(&self) -> Vec<DialInfo> {
        self.dial_info_entries
            .iter()
            .filter_map(|e| {
                if e.is_private() {
                    Some(e.dial_info().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn local_dial_info_for_protocol(&mut self, protocol_type: ProtocolType) -> Vec<DialInfo> {
        self.dial_info_entries
            .iter_mut()
            .filter_map(|e| {
                if e.dial_info().protocol_type() != protocol_type {
                    None
                } else if e.is_private() {
                    Some(e.dial_info().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_peer_info(&self, key: DHTKey, scope: PeerScope) -> PeerInfo {
        PeerInfo {
            node_id: NodeId::new(key),
            dial_infos: match scope {
                PeerScope::All => self.dial_info(),
                PeerScope::Global => self.global_dial_info(),
                PeerScope::Local => self.local_dial_info(),
            },
        }
    }

    pub fn set_last_connection(&mut self, last_connection: ConnectionDescriptor, timestamp: u64) {
        self.last_connection = Some((last_connection, timestamp));

        // sort the dialinfoentries by the last peer address if we have a match
        // if one particular peer address is being used and matches a dialinfoentry
        // then we should prefer it
        for i in 0..self.dial_info_entries.len() {
            let die = &mut self.dial_info_entries[i];

            // see if we have a matching address
            if RoutingTable::dial_info_peer_address_match(
                die.dial_info(),
                &self.last_connection.as_ref().unwrap().0.remote,
            ) {
                // push the most recent dialinfo to the front
                let dies = &mut self.dial_info_entries;
                let die = dies.remove(i).unwrap();
                dies.push_front(die);

                break;
            }
        }
    }

    pub fn last_connection(&self) -> Option<ConnectionDescriptor> {
        self.last_connection.as_ref().map(|x| x.0.clone())
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
            Some(ts) => (cur_ts - ts) >= (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64),
        }
    }
    pub(super) fn check_dead(&self, cur_ts: u64) -> bool {
        // if we have not heard from the node at all for the duration of the unreliable ping span
        match self.peer_stats.last_seen {
            None => true,
            Some(ts) => (cur_ts - ts) >= (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64),
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
                            + (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64);
                        let reliable_cur = cur_ts - start_of_reliable_time;
                        let reliable_last = last_pinged - start_of_reliable_time;

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
                        (cur_ts - last_pinged)
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

    ////////////////////////////////////////////////////////////////
    /// Called when rpc processor things happen

    pub(super) fn ping_sent(&mut self, ts: u64, bytes: u64) {
        self.peer_stats.ping_stats.total_sent += 1;
        self.stats_accounting.add_up(bytes);
        self.peer_stats.ping_stats.in_flight += 1;
        self.peer_stats.ping_stats.last_pinged = Some(ts);
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
    pub(super) fn question_sent(&mut self, _ts: u64, bytes: u64) {
        self.stats_accounting.add_up(bytes);
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
