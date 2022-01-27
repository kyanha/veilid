use super::*;
use crate::intf::*;
use core::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::OnceCell;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        type TickTaskRoutine =
            dyn Fn(u64, u64) -> PinBoxFuture<Result<(), String>> + 'static;
    } else {
        type TickTaskRoutine =
            dyn Fn(u64, u64) -> SendPinBoxFuture<Result<(), String>> + Send + Sync + 'static;
    }
}

/// Runs a single-future background processing task, attempting to run it once every 'tick period' microseconds.
/// If the prior tick is still running, it will allow it to finish, and do another tick when the timer comes around again.
/// One should attempt to make tasks short-lived things that run in less than the tick period if you want things to happen with regular periodicity.
pub struct TickTask {
    last_timestamp_us: AtomicU64,
    tick_period_us: u64,
    routine: OnceCell<Box<TickTaskRoutine>>,
    single_future: SingleFuture<Result<(), String>>,
}

impl TickTask {
    pub fn new_us(tick_period_us: u64) -> Self {
        Self {
            last_timestamp_us: AtomicU64::new(0),
            tick_period_us,
            routine: OnceCell::new(),
            single_future: SingleFuture::new(),
        }
    }
    pub fn new_ms(tick_period_ms: u32) -> Self {
        Self {
            last_timestamp_us: AtomicU64::new(0),
            tick_period_us: (tick_period_ms as u64) * 1000u64,
            routine: OnceCell::new(),
            single_future: SingleFuture::new(),
        }
    }
    pub fn new(tick_period_sec: u32) -> Self {
        Self {
            last_timestamp_us: AtomicU64::new(0),
            tick_period_us: (tick_period_sec as u64) * 1000000u64,
            routine: OnceCell::new(),
            single_future: SingleFuture::new(),
        }
    }

    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            pub fn set_routine(
                &self,
                routine: impl Fn(u64, u64) -> PinBoxFuture<Result<(), String>> + 'static,
            ) {
                self.routine.set(Box::new(routine)).map_err(drop).unwrap();
            }
        } else {
            pub fn set_routine(
                &self,
                routine: impl Fn(u64, u64) -> SendPinBoxFuture<Result<(), String>> + Send + Sync + 'static,
            ) {
                self.routine.set(Box::new(routine)).map_err(drop).unwrap();
            }
        }
    }

    pub async fn cancel(&self) -> Result<(), String> {
        match self.single_future.cancel().await {
            Ok(Some(Err(err))) => Err(err),
            _ => Ok(()),
        }
    }

    pub async fn tick(&self) -> Result<(), String> {
        let now = get_timestamp();
        let last_timestamp_us = self.last_timestamp_us.load(Ordering::Acquire);

        if last_timestamp_us == 0u64 || (now - last_timestamp_us) >= self.tick_period_us {
            // Run the singlefuture
            match self
                .single_future
                .single_spawn(self.routine.get().unwrap()(last_timestamp_us, now))
                .await
            {
                Ok(Some(Err(err))) => {
                    // If the last execution errored out then we should pass that error up
                    self.last_timestamp_us.store(now, Ordering::Release);
                    return Err(err);
                }
                Ok(None) | Err(()) => {
                    // If the execution didn't happen this time because it was already running
                    // then we should try again the next tick and not reset the timestamp so we try as soon as possible
                }
                _ => {
                    // Execution happened, next execution attempt should happen only after tick period
                    self.last_timestamp_us.store(now, Ordering::Release);
                }
            }
        }

        Ok(())
    }
}
