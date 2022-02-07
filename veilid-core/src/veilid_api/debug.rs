////////////////////////////////////////////////////////////////
// Debugging

use super::*;
use routing_table::*;

fn get_bucket_entry_state(text: &str) -> Option<BucketEntryState> {
    if text == "dead" {
        Some(BucketEntryState::Dead)
    } else if text == "reliable" {
        Some(BucketEntryState::Reliable)
    } else if text == "unreliable" {
        Some(BucketEntryState::Unreliable)
    } else {
        None
    }
}
fn get_number(text: &str) -> Option<usize> {
    usize::from_str(text).ok()
}
fn get_dht_key(text: &str) -> Option<DHTKey> {
    DHTKey::try_decode(text).ok()
}
fn get_debug_argument<T, G: FnOnce(&str) -> Option<T>>(
    value: &str,
    context: &str,
    argument: &str,
    getter: G,
) -> Result<T, VeilidAPIError> {
    if let Some(val) = getter(value) {
        Ok(val)
    } else {
        Err(VeilidAPIError::InvalidArgument {
            context: context.to_owned(),
            argument: argument.to_owned(),
            value: value.to_owned(),
        })
    }
}
fn get_debug_argument_at<T, G: FnOnce(&str) -> Option<T>>(
    debug_args: &[String],
    pos: usize,
    context: &str,
    argument: &str,
    getter: G,
) -> Result<T, VeilidAPIError> {
    if pos >= debug_args.len() {
        return Err(VeilidAPIError::MissingArgument {
            context: context.to_owned(),
            argument: argument.to_owned(),
        });
    }
    let value = &debug_args[pos];
    if let Some(val) = getter(value) {
        Ok(val)
    } else {
        Err(VeilidAPIError::InvalidArgument {
            context: context.to_owned(),
            argument: argument.to_owned(),
            value: value.to_owned(),
        })
    }
}

impl VeilidAPI {
    async fn debug_buckets(&self, debug_args: &[String]) -> Result<String, VeilidAPIError> {
        let mut min_state = BucketEntryState::Unreliable;
        if debug_args.len() == 1 {
            min_state = get_debug_argument(
                &debug_args[0],
                "debug_buckets",
                "min_state",
                get_bucket_entry_state,
            )?;
        }
        // Dump routing table bucket info
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        Ok(routing_table.debug_info_buckets(min_state))
    }

    async fn debug_dialinfo(&self, _debug_args: &[String]) -> Result<String, VeilidAPIError> {
        // Dump routing table dialinfo
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        Ok(routing_table.debug_info_dialinfo())
    }

    async fn debug_entries(&self, debug_args: &[String]) -> Result<String, VeilidAPIError> {
        let mut min_state = BucketEntryState::Unreliable;
        let mut limit = 20;
        for arg in debug_args {
            if let Some(ms) = get_bucket_entry_state(arg) {
                min_state = ms;
            } else if let Some(lim) = get_number(arg) {
                limit = lim;
            } else {
                return Err(VeilidAPIError::InvalidArgument {
                    context: "debug_entries".to_owned(),
                    argument: "unknown".to_owned(),
                    value: arg.clone(),
                });
            }
        }

        // Dump routing table entries
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        Ok(routing_table.debug_info_entries(limit, min_state))
    }

    async fn debug_entry(&self, debug_args: &[String]) -> Result<String, VeilidAPIError> {
        let node_id = get_debug_argument_at(debug_args, 0, "debug_entry", "node_id", get_dht_key)?;

        // Dump routing table entry
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        Ok(routing_table.debug_info_entry(node_id))
    }

    async fn debug_nodeinfo(&self, _debug_args: &[String]) -> Result<String, VeilidAPIError> {
        // Dump routing table entry
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        Ok(routing_table.debug_info_nodeinfo())
    }

    pub async fn debug(&self, what: String) -> Result<String, VeilidAPIError> {
        trace!("VeilidCore::debug");
        let debug_args: Vec<String> = what
            .split_ascii_whitespace()
            .map(|s| s.to_owned())
            .collect();
        if debug_args.is_empty() {
            return Ok(r#">>> Debug commands:
    buckets [dead|reliable]
    dialinfo
    entries [dead|reliable] [limit]
    entry [node_id]
    nodeinfo
"#
            .to_owned());
        }
        let mut out = String::new();
        let arg = &debug_args[0];
        if arg == "buckets" {
            out += self.debug_buckets(&debug_args[1..]).await?.as_str();
        } else if arg == "dialinfo" {
            out += self.debug_dialinfo(&debug_args[1..]).await?.as_str();
        } else if arg == "entries" {
            out += self.debug_entries(&debug_args[1..]).await?.as_str();
        } else if arg == "entry" {
            out += self.debug_entry(&debug_args[1..]).await?.as_str();
        } else if arg == "nodeinfo" {
            out += self.debug_nodeinfo(&debug_args[1..]).await?.as_str();
        } else {
            out += ">>> Unknown command\n";
        }
        Ok(out)
    }
}
