use super::*;
use crate::*;
use core::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::OnceCell;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        type TickTaskRoutine<E> =
            dyn Fn(StopToken, u64, u64) -> PinBoxFuture<Result<(), E>> + 'static;
    } else {
        type TickTaskRoutine<E> =
            dyn Fn(StopToken, u64, u64) -> SendPinBoxFuture<Result<(), E>> + Send + Sync + 'static;
    }
}

/// Runs a single-future background processing task, attempting to run it once every 'tick period' microseconds.
/// If the prior tick is still running, it will allow it to finish, and do another tick when the timer comes around again.
/// One should attempt to make tasks short-lived things that run in less than the tick period if you want things to happen with regular periodicity.
pub struct TickTask<
    #[cfg(target_arch = "wasm32")] E: 'static,
    #[cfg(not(target_arch = "wasm32"))] E: Send + 'static,
> {
    last_timestamp_us: AtomicU64,
    tick_period_us: u64,
    routine: OnceCell<Box<TickTaskRoutine<E>>>,
    stop_source: AsyncMutex<Option<StopSource>>,
    single_future: MustJoinSingleFuture<Result<(), E>>,
}

impl<
        #[cfg(target_arch = "wasm32")] E: 'static,
        #[cfg(not(target_arch = "wasm32"))] E: Send + 'static,
    > TickTask<E>
{
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

    pub fn set_routine(
        &self,
        routine: impl Fn(StopToken, u64, u64) -> SendPinBoxFuture<Result<(), E>> + Send + Sync + 'static,
    ) {
        self.routine.set(Box::new(routine)).map_err(drop).unwrap();
    }

    pub async fn stop(&self) -> Result<(), E> {
        // drop the stop source if we have one
        let opt_stop_source = &mut *self.stop_source.lock().await;
        if opt_stop_source.is_none() {
            // already stopped, just return
            trace!("tick task already stopped");
            return Ok(());
        }
        drop(opt_stop_source.take());

        // wait for completion of the tick task
        trace!("stopping single future");
        match self.single_future.join().await {
            Ok(Some(Err(err))) => Err(err),
            _ => Ok(()),
        }
    }

    pub async fn tick(&self) -> Result<(), E> {
        let now = intf::get_timestamp();
        let last_timestamp_us = self.last_timestamp_us.load(Ordering::Acquire);

        if last_timestamp_us != 0u64 && (now - last_timestamp_us) < self.tick_period_us {
            // It's not time yet
            return Ok(());
        }

        // Lock the stop source, tells us if we have ever started this future
        let opt_stop_source = &mut *self.stop_source.lock().await;
        if opt_stop_source.is_some() {
            // See if the previous execution finished with an error
            match self.single_future.check().await {
                Ok(Some(Err(e))) => {
                    // We have an error result, which means the singlefuture ran but we need to propagate the error
                    return Err(e);
                }
                Ok(Some(Ok(()))) => {
                    // We have an ok result, which means the singlefuture ran, and we should run it again this tick
                }
                Ok(None) => {
                    // No prior result to return which means things are still running
                    // We can just return now, since the singlefuture will not run a second time
                    return Ok(());
                }
                Err(()) => {
                    // If we get this, it's because we are joining the singlefuture already
                    // Don't bother running but this is not an error in this case
                    return Ok(());
                }
            };
        }

        // Run the singlefuture
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
            // We should have already consumed the result of the last run, or there was none
            // and we should definitely have run, because the prior 'check()' operation
            // should have ensured the singlefuture was ready to run
            Ok((None, true)) => {
                // Set new timer
                self.last_timestamp_us.store(now, Ordering::Release);
                // Save new stopper
                *opt_stop_source = Some(stop_source);
                Ok(())
            }
            // All other conditions should not be reachable
            _ => {
                unreachable!();
            }
        }
    }
}
