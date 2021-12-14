use crate::xx::*;
use alloc::string::ToString;

#[macro_export]
macro_rules! assert_err {
    ($ex:expr) => {
        if let Ok(v) = $ex {
            panic!("assertion failed, expected Err(..), got {:?}", v);
        }
    };
}

pub fn split_port(name: &str) -> Result<(String, Option<u16>), String> {
    if let Some(split) = name.rfind(':') {
        let hoststr = &name[0..split];
        let portstr = &name[split + 1..];
        let port: u16 = portstr
            .parse::<u16>()
            .map_err(|e| format!("Invalid port: {}", e))?;

        Ok((hoststr.to_string(), Some(port)))
    } else {
        Ok((name.to_string(), None))
    }
}

pub fn prepend_slash(s: String) -> String {
    if s.starts_with('/') {
        return s;
    }
    let mut out = "/".to_owned();
    out.push_str(s.as_str());
    out
}

pub fn timestamp_to_secs(ts: u64) -> f64 {
    ts as f64 / 1000000.0f64
}

pub fn secs_to_timestamp(secs: f64) -> u64 {
    (secs * 1000000.0f64) as u64
}

// Calculate retry attempt with logarhythmic falloff
pub fn retry_falloff_log(
    last_us: u64,
    cur_us: u64,
    interval_start_us: u64,
    interval_max_us: u64,
    interval_multiplier_us: f64,
) -> bool {
    //
    if cur_us < interval_start_us {
        // Don't require a retry within the first 'interval_start_us' microseconds of the reliable time period
        false
    } else if cur_us >= last_us + interval_max_us {
        // Retry at least every 'interval_max_us' microseconds
        true
    } else {
        // Exponential falloff between 'interval_start_us' and 'interval_max_us' microseconds
        // Optimal equation here is: y = Sum[Power[b,x],{n,0,x}] --> y = (x+1)b^x
        // but we're just gonna simplify this to a log curve for speed
        let last_secs = timestamp_to_secs(last_us);
        let nth = (last_secs / timestamp_to_secs(interval_start_us))
            .log(interval_multiplier_us)
            .floor() as i32;
        let next_secs = timestamp_to_secs(interval_start_us) * interval_multiplier_us.powi(nth + 1);
        let next_us = secs_to_timestamp(next_secs);
        cur_us >= next_us
    }
}

pub fn try_at_most_n_things<T, I, C, R>(max: usize, things: I, closure: C) -> Option<R>
where
    I: IntoIterator<Item = T>,
    C: Fn(T) -> Option<R>,
{
    let mut fails = 0usize;
    for thing in things.into_iter() {
        if let Some(r) = closure(thing) {
            return Some(r);
        }
        fails += 1;
        if fails >= max {
            break;
        }
    }
    None
}

pub async fn async_try_at_most_n_things<T, I, C, R, F>(
    max: usize,
    things: I,
    closure: C,
) -> Option<R>
where
    I: IntoIterator<Item = T>,
    C: Fn(T) -> F,
    F: Future<Output = Option<R>>,
{
    let mut fails = 0usize;
    for thing in things.into_iter() {
        if let Some(r) = closure(thing).await {
            return Some(r);
        }
        fails += 1;
        if fails >= max {
            break;
        }
    }
    None
}

pub trait CmpAssign {
    fn min_assign(&mut self, other: Self);
    fn max_assign(&mut self, other: Self);
}

impl<T> CmpAssign for T
where
    T: core::cmp::Ord,
{
    fn min_assign(&mut self, other: Self) {
        if &other < self {
            *self = other;
        }
    }
    fn max_assign(&mut self, other: Self) {
        if &other > self {
            *self = other;
        }
    }
}
