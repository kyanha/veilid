use super::*;
use crate::intf::*;
use core::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::OnceCell;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        type TickTaskRoutine =
            dyn Fn(StopToken, u64, u64) -> PinBoxFuture<Result<(), String>> + 'static;
    } else {
        type TickTaskRoutine =
            dyn Fn(StopToken, u64, u64) -> SendPinBoxFuture<Result<(), String>> + Send + Sync + 'static;
    }
}

/// Runs a single-future background processing task, attempting to run it once every 'tick period' microseconds.
/// If the prior tick is still running, it will allow it to finish, and do another tick when the timer comes around again.
/// One should attempt to make tasks short-lived things that run in less than the tick period if you want things to happen with regular periodicity.
pub struct TickTask {
    last_timestamp_us: AtomicU64,
    tick_period_us: u64,
    routine: OnceCell<Box<TickTaskRoutine>>,
    stop_source: AsyncMutex<Option<StopSource>>,
    single_future: MustJoinSingleFuture<Result<(), String>>,
}

impl TickTask {
    pub fn new_us(tick_period_us: u64) -> Self {
        Self {
            last_timestamp_us: AtomicU64::new(0),
            tick_period_us,
            routine: OnceCell::new(),
            stop_source: AsyncMutex::new(None),
            single_future: MustJoinSingleFuture::new(),
        }
    }
    pub fn new_ms(tick_period_ms: u32) -> Self {
        Self {
            last_timestamp_us: AtomicU64::new(0),
            tick_period_us: (tick_period_ms as u64) * 1000u64,
            routine: OnceCell::new(),
            stop_source: AsyncMutex::new(None),
            single_future: MustJoinSingleFuture::new(),
        }
    }
    pub fn new(tick_period_sec: u32) -> Self {
        Self {
            last_timestamp_us: AtomicU64::new(0),
            tick_period_us: (tick_period_sec as u64) * 1000000u64,
            routine: OnceCell::new(),
            stop_source: AsyncMutex::new(None),
            single_future: MustJoinSingleFuture::new(),
        }
    }

    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            pub fn set_routine(
                &self,
                routine: impl Fn(StopToken, u64, u64) -> PinBoxFuture<Result<(), String>> + 'static,
            ) {
                self.routine.set(Box::new(routine)).map_err(drop).unwrap();
            }
        } else {
            pub fn set_routine(
                &self,
                routine: impl Fn(StopToken, u64, u64) -> SendPinBoxFuture<Result<(), String>> + Send + Sync + 'static,
            ) {
                self.routine.set(Box::new(routine)).map_err(drop).unwrap();
            }
        }
    }

    pub async fn stop(&self) -> Result<(), String> {
        // drop the stop source if we have one
        let opt_stop_source = &mut *self.stop_source.lock().await;
        if opt_stop_source.is_none() {
            // already stopped, just return
            return Ok(());
        }
        *opt_stop_source = None;

        // wait for completion of the tick task
        match self.single_future.join().await {
            Ok(Some(Err(err))) => Err(err),
            _ => Ok(()),
        }
    }

    pub async fn tick(&self) -> Result<(), String> {
        let now = get_timestamp();
        let last_timestamp_us = self.last_timestamp_us.load(Ordering::Acquire);

        if last_timestamp_us == 0u64 || (now - last_timestamp_us) >= self.tick_period_us {
            // Run the singlefuture
            let opt_stop_source = &mut *self.stop_source.lock().await;
            let stop_source = StopSource::new();
            match self
                .single_future
                .single_spawn(self.routine.get().unwrap()(
                    stop_source.token(),
                    last_timestamp_us,
                    now,
                ))
                .await
            {
                // Single future ran this tick
                Ok(Some(ret)) => {
                    // Set new timer
                    self.last_timestamp_us.store(now, Ordering::Release);
                    // Save new stopper
                    *opt_stop_source = Some(stop_source);
                    ret
                }
                // Single future did not run this tick
                Ok(None) | Err(()) => {
                    // If the execution didn't happen this time because it was already running
                    // then we should try again the next tick and not reset the timestamp so we try as soon as possible
                    Ok(())
                }
            }
        } else {
            // It's not time yet
            Ok(())
        }
    }
}
