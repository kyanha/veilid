use super::*;

#[derive(Clone)]
pub struct Bucket {
    routing_table: RoutingTable,
    entries: BTreeMap<DHTKey, BucketEntry>,
    newest_entry: Option<DHTKey>,
}
pub(super) type EntriesIterMut<'a> =
    alloc::collections::btree_map::IterMut<'a, DHTKey, BucketEntry>;
pub(super) type EntriesIter<'a> = alloc::collections::btree_map::Iter<'a, DHTKey, BucketEntry>;

fn state_ordering(state: BucketEntryState) -> usize {
    match state {
        BucketEntryState::Dead => 0,
        BucketEntryState::Unreliable => 1,
        BucketEntryState::Reliable => 2,
    }
}

impl Bucket {
    pub fn new(routing_table: RoutingTable) -> Self {
        Self {
            routing_table: routing_table,
            entries: BTreeMap::new(),
            newest_entry: None,
        }
    }

    pub(super) fn add_entry(&mut self, node_id: DHTKey) -> NodeRef {
        info!("Node added: {}", node_id.encode());

        // Add new entry
        self.entries.insert(node_id, BucketEntry::new());

        // This is now the newest bucket entry
        self.newest_entry = Some(node_id);

        // Get a node ref to return
        let entry_ref = self.entries.get_mut(&node_id).unwrap();
        NodeRef::new(self.routing_table.clone(), node_id, entry_ref)
    }

    pub(super) fn remove_entry(&mut self, node_id: &DHTKey) {
        info!("Node removed: {}", node_id);

        // Remove the entry
        self.entries.remove(node_id);

        // newest_entry is updated by kick_bucket()
    }

    pub(super) fn roll_transfers(&mut self, last_ts: u64, cur_ts: u64) {
        // Called every ROLLING_TRANSFERS_INTERVAL_SECS
        for entry in &mut self.entries {
            entry.1.roll_transfers(last_ts, cur_ts);
        }
    }

    pub(super) fn entry_mut(&mut self, key: &DHTKey) -> Option<&mut BucketEntry> {
        self.entries.get_mut(key)
    }

    pub(super) fn entries(&self) -> EntriesIter {
        self.entries.iter()
    }

    pub(super) fn entries_mut(&mut self) -> EntriesIterMut {
        self.entries.iter_mut()
    }

    pub(super) fn kick(&mut self, bucket_depth: usize) -> Option<BTreeSet<DHTKey>> {
        // Get number of entries to attempt to purge from bucket
        let bucket_len = self.entries.len();
        if bucket_len <= bucket_depth {
            return None;
        }
        // Try to purge the newest entries that overflow the bucket
        let mut dead_node_ids: BTreeSet<DHTKey> = BTreeSet::new();
        let mut extra_entries = bucket_len - bucket_depth;

        // Get the sorted list of entries by their kick order
        let mut sorted_entries: Vec<(&_, &_)> = self.entries.iter().collect();
        let cur_ts = get_timestamp();
        sorted_entries.sort_by(
            |a: &(&DHTKey, &BucketEntry), b: &(&DHTKey, &BucketEntry)| -> core::cmp::Ordering {
                let ea = a.1;
                let eb = b.1;
                let astate = state_ordering(ea.state(cur_ts));
                let bstate = state_ordering(eb.state(cur_ts));
                // first kick dead nodes, then unreliable nodes
                if astate < bstate {
                    return core::cmp::Ordering::Less;
                }
                if astate > bstate {
                    return core::cmp::Ordering::Greater;
                }
                // then kick by time added, most recent nodes are kicked first
                let ata = ea.peer_stats().time_added;
                let bta = eb.peer_stats().time_added;
                bta.cmp(&ata)
            },
        );

        self.newest_entry = None;
        for i in 0..sorted_entries.len() {
            // If we're not evicting more entries, exit, noting this may be the newest entry
            if extra_entries == 0 {
                // The first 'live' entry we find is our newest entry
                if self.newest_entry.is_none() {
                    self.newest_entry = Some(sorted_entries[i].0.clone());
                }
                break;
            }
            extra_entries -= 1;

            // if this entry has references we can't drop it yet
            if sorted_entries[i].1.ref_count > 0 {
                // The first 'live' entry we fine is our newest entry
                if self.newest_entry.is_none() {
                    self.newest_entry = Some(sorted_entries[i].0.clone());
                }
                continue;
            }

            // if no references, lets evict it
            dead_node_ids.insert(sorted_entries[i].0.clone());
        }

        // Now purge the dead node ids
        for id in &dead_node_ids {
            // Remove the entry
            self.remove_entry(id);
        }

        if dead_node_ids.len() > 0 {
            Some(dead_node_ids)
        } else {
            None
        }
    }
}
