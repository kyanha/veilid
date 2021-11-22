use super::*;

// Latency entry is per round-trip packet (ping or data)
// - Size is number of entries
const ROLLING_LATENCIES_SIZE: usize = 10;

// Transfers entries are in bytes total for the interval
// - Size is number of entries
// - Interval is number of seconds in each entry
const ROLLING_TRANSFERS_SIZE: usize = 10;
pub const ROLLING_TRANSFERS_INTERVAL_SECS: u32 = 10;

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
    rolling_latencies: VecDeque<u64>,
    rolling_transfers: VecDeque<(u64, u64)>,
    current_transfer: (u64, u64),
    peer_stats: PeerStats,
}

impl BucketEntry {
    pub(super) fn new() -> Self {
        Self {
            ref_count: 0,
            min_max_version: None,
            last_connection: None,
            dial_info_entries: VecDeque::new(),
            rolling_latencies: VecDeque::new(),
            rolling_transfers: VecDeque::new(),
            current_transfer: (0, 0),
            peer_stats: PeerStats {
                time_added: get_timestamp(),
                last_seen: None,
                ping_stats: PingStats::default(),
                latency: None,
                transfer: (TransferStats::default(), TransferStats::default()),
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

    pub fn public_dial_info(&self) -> Vec<DialInfo> {
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

    pub fn public_dial_info_for_protocol(&self, protocol_type: ProtocolType) -> Vec<DialInfo> {
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

    pub fn private_dial_info(&self) -> Vec<DialInfo> {
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

    pub fn private_dial_info_for_protocol(&mut self, protocol_type: ProtocolType) -> Vec<DialInfo> {
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
                PeerScope::Public => self.public_dial_info(),
                PeerScope::Private => self.private_dial_info(),
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
                drop(die);

                // push the most recent dialinfo to the front
                let dies = &mut self.dial_info_entries;
                let die = dies.remove(i).unwrap();
                dies.push_front(die);

                break;
            }
        }
    }

    pub fn last_connection(&self) -> Option<ConnectionDescriptor> {
        match self.last_connection.as_ref() {
            Some(x) => Some(x.0.clone()),
            None => None,
        }
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
        let dur_ms = (cur_ts - last_ts) / 1000u64;
        while self.rolling_transfers.len() >= ROLLING_TRANSFERS_SIZE {
            self.rolling_transfers.pop_front();
        }
        self.rolling_transfers.push_back(self.current_transfer);
        self.current_transfer = (0, 0);

        let xd = &mut self.peer_stats.transfer.0;
        let xu = &mut self.peer_stats.transfer.1;

        xd.maximum = 0;
        xu.maximum = 0;
        xd.minimum = u64::MAX;
        xu.minimum = u64::MAX;
        xd.average = 0;
        xu.average = 0;
        for (rtd, rtu) in &self.rolling_transfers {
            let bpsd = rtd * 1000u64 / dur_ms;
            let bpsu = rtu * 1000u64 / dur_ms;
            if bpsd > xd.maximum {
                xd.maximum = bpsd;
            }
            if bpsu > xu.maximum {
                xu.maximum = bpsu;
            }
            if bpsd < xd.minimum {
                xd.minimum = bpsd;
            }
            if bpsu < xu.minimum {
                xu.minimum = bpsu;
            }
            xd.average += bpsd;
            xu.average += bpsu;
        }
        let len = self.rolling_transfers.len() as u64;
        xd.average /= len;
        xu.average /= len;
        // total remains unchanged
    }

    // Called for every round trip packet we receive
    fn record_latency(&mut self, latency: u64) {
        while self.rolling_latencies.len() >= ROLLING_LATENCIES_SIZE {
            self.rolling_latencies.pop_front();
        }
        self.rolling_latencies.push_back(latency);

        let mut ls = LatencyStats {
            fastest: 0,
            average: 0,
            slowest: 0,
        };
        for rl in &self.rolling_latencies {
            if *rl < ls.fastest {
                ls.fastest = *rl;
            }
            if *rl > ls.slowest {
                ls.slowest = *rl;
            }
            ls.average += *rl;
        }
        let len = self.rolling_latencies.len() as u64;
        ls.average /= len;

        self.peer_stats.latency = Some(ls);
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
    /// Called by RPC processor as events happen

    pub fn ping_sent(&mut self, ts: u64, bytes: u64) {
        self.peer_stats.ping_stats.total_sent += 1;
        self.current_transfer.1 += bytes;
        self.peer_stats.ping_stats.in_flight += 1;
        self.peer_stats.ping_stats.last_pinged = Some(ts);
    }
    pub fn ping_rcvd(&mut self, ts: u64, bytes: u64) {
        self.current_transfer.0 += bytes;
        self.touch_last_seen(ts);
    }
    pub fn pong_sent(&mut self, _ts: u64, bytes: u64) {
        self.current_transfer.1 += bytes;
    }
    pub fn pong_rcvd(&mut self, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.current_transfer.0 += bytes;
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
    pub fn ping_lost(&mut self, _ts: u64) {
        self.peer_stats.ping_stats.in_flight -= 1;
        self.peer_stats.ping_stats.recent_lost_pings += 1;
        self.peer_stats.ping_stats.consecutive_pongs = 0;
        self.peer_stats.ping_stats.first_consecutive_pong_time = None;
    }
    pub fn question_sent(&mut self, _ts: u64, bytes: u64) {
        self.current_transfer.1 += bytes;
    }
    pub fn question_rcvd(&mut self, ts: u64, bytes: u64) {
        self.current_transfer.0 += bytes;
        self.touch_last_seen(ts);
    }
    pub fn answer_sent(&mut self, _ts: u64, bytes: u64) {
        self.current_transfer.1 += bytes;
    }
    pub fn answer_rcvd(&mut self, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.current_transfer.0 += bytes;
        self.record_latency(recv_ts - send_ts);
        self.touch_last_seen(recv_ts);
    }
    pub fn question_lost(&mut self, _ts: u64) {
        self.peer_stats.ping_stats.consecutive_pongs = 0;
        self.peer_stats.ping_stats.first_consecutive_pong_time = None;
    }
}

impl Drop for BucketEntry {
    fn drop(&mut self) {
        assert_eq!(self.ref_count, 0);
    }
}
