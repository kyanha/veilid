use super::*;

impl RoutingTable {
    pub fn debug_info_nodeinfo(&self) -> String {
        let mut out = String::new();
        let inner = self.inner.lock();
        out += "Routing Table Info:\n";

        out += &format!("   Node Id: {}\n", inner.node_id.encode());
        out += &format!(
            "   Self Latency Stats Accounting: {:#?}\n\n",
            inner.self_latency_stats_accounting
        );
        out += &format!(
            "   Self Transfer Stats Accounting: {:#?}\n\n",
            inner.self_transfer_stats_accounting
        );
        out += &format!(
            "   Self Transfer Stats: {:#?}\n\n",
            inner.self_transfer_stats
        );

        out
    }
    pub fn debug_info_dialinfo(&self) -> String {
        let ldis = self.local_dial_info_details();
        let gdis = self.global_dial_info_details();
        let mut out = String::new();

        out += "Local Dial Info Details:\n";
        for (n, ldi) in ldis.iter().enumerate() {
            out += &format!("  {:>2}: {:?}\n", n, ldi);
        }
        out += "Global Dial Info Details:\n";
        for (n, gdi) in gdis.iter().enumerate() {
            out += &format!("  {:>2}: {:?}\n", n, gdi);
        }
        out
    }
    pub fn debug_info_entries(&self, limit: usize, min_state: BucketEntryState) -> String {
        let inner = self.inner.lock();
        let cur_ts = get_timestamp();

        let mut out = String::new();

        let blen = inner.buckets.len();
        let mut b = 0;
        let mut cnt = 0;
        out += &format!("Entries: {}\n", inner.bucket_entry_count);
        while b < blen {
            if inner.buckets[b].entries().len() > 0 {
                out += &format!("  Bucket #{}:\n", b);
                for e in inner.buckets[b].entries() {
                    let state = e.1.state(cur_ts);
                    if state >= min_state {
                        out += &format!(
                            "    {} [{}]\n",
                            e.0.encode(),
                            match state {
                                BucketEntryState::Reliable => "R",
                                BucketEntryState::Unreliable => "U",
                                BucketEntryState::Dead => "D",
                            }
                        );

                        cnt += 1;
                        if cnt >= limit {
                            break;
                        }
                    }
                }
                if cnt >= limit {
                    break;
                }
            }
            b += 1;
        }

        out
    }

    pub fn debug_info_entry(&self, node_id: DHTKey) -> String {
        let mut out = String::new();
        out += &format!("Entry {:?}:\n", node_id);
        if let Some(nr) = self.lookup_node_ref(node_id) {
            out += &nr.operate(|e| format!("{:#?}\n", e));
        } else {
            out += "Entry not found\n";
        }

        out
    }

    pub fn debug_info_buckets(&self, min_state: BucketEntryState) -> String {
        let inner = self.inner.lock();
        let cur_ts = get_timestamp();

        let mut out = String::new();
        const COLS: usize = 16;
        let rows = inner.buckets.len() / COLS;
        let mut r = 0;
        let mut b = 0;
        out += "Buckets:\n";
        while r < rows {
            let mut c = 0;
            out += format!("  {:>3}: ", b).as_str();
            while c < COLS {
                let mut cnt = 0;
                for e in inner.buckets[b].entries() {
                    if e.1.state(cur_ts) >= min_state {
                        cnt += 1;
                    }
                }
                out += format!("{:>3} ", cnt).as_str();
                b += 1;
                c += 1;
            }
            out += "\n";
            r += 1;
        }

        out
    }
}
