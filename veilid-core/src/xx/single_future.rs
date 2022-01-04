use super::*;
use crate::intf::*;
use cfg_if::*;
use core::task::Poll;
use futures_util::poll;

#[derive(Debug)]
struct SingleFutureInner<T>
where
    T: 'static,
{
    locked: bool,
    join_handle: Option<JoinHandle<T>>,
}

/// Spawns a single background processing task idempotently, possibly returning the return value of the previously executed background task
/// This does not queue, just ensures that no more than a single copy of the task is running at a time, but allowing tasks to be retriggered
#[derive(Debug, Clone)]
pub struct SingleFuture<T>
where
    T: 'static,
{
    inner: Arc<Mutex<SingleFutureInner<T>>>,
}

impl<T> Default for SingleFuture<T>
where
    T: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SingleFuture<T>
where
    T: 'static,
{
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SingleFutureInner {
                locked: false,
                join_handle: None,
            })),
        }
    }

    fn try_lock(&self) -> Result<Option<JoinHandle<T>>, ()> {
        let mut inner = self.inner.lock();
        if inner.locked {
            // If already locked error out
            return Err(());
        }
        inner.locked = true;
        // If we got the lock, return what we have for a join handle if anything
        Ok(inner.join_handle.take())
    }

    fn unlock(&self, jh: Option<JoinHandle<T>>) {
        let mut inner = self.inner.lock();
        assert!(inner.locked);
        assert!(inner.join_handle.is_none());
        inner.locked = false;
        inner.join_handle = jh;
    }

    // Check the result
    pub async fn check(&self) -> Result<Option<T>, ()> {
        let mut out: Option<T> = None;

        // See if we have a result we can return
        let maybe_jh = match self.try_lock() {
            Ok(v) => v,
            Err(_) => {
                // If we are already polling somewhere else, don't hand back a result
                return Err(());
            }
        };
        if maybe_jh.is_some() {
            let mut jh = maybe_jh.unwrap();

            // See if we finished, if so, return the value of the last execution
            if let Poll::Ready(r) = poll!(&mut jh) {
                out = Some(r);
                // Task finished, unlock with nothing
                self.unlock(None);
            } else {
                // Still running put the join handle back so we can check on it later
                self.unlock(Some(jh));
            }
        } else {
            // No task, unlock with nothing
            self.unlock(None);
        }

        // Return the prior result if we have one
        Ok(out)
    }

    // Wait for the result
    pub async fn join(&self) -> Result<Option<T>, ()> {
        let mut out: Option<T> = None;

        // See if we have a result we can return
        let maybe_jh = match self.try_lock() {
            Ok(v) => v,
            Err(_) => {
                // If we are already polling somewhere else,
                // that's an error because you can only join
                // these things once
                return Err(());
            }
        };
        if maybe_jh.is_some() {
            let jh = maybe_jh.unwrap();
            // Wait for return value of the last execution
            out = Some(jh.await);
            // Task finished, unlock with nothing
        } else {
            // No task, unlock with nothing
        }
        self.unlock(None);

        // Return the prior result if we have one
        Ok(out)
    }

    // Cancel
    pub async fn cancel(&self) -> Result<Option<T>, ()> {
        let mut out: Option<T> = None;

        // See if we have a result we can return
        let maybe_jh = match self.try_lock() {
            Ok(v) => v,
            Err(_) => {
                // If we are already polling somewhere else, don't hand back a result
                return Err(());
            }
        };
        if maybe_jh.is_some() {
            let mut jh = maybe_jh.unwrap();

            // See if we finished, if so, return the value of the last execution
            if let Poll::Ready(r) = poll!(&mut jh) {
                out = Some(r);
                // Task finished, unlock with nothing
            } else {
                // Still running but drop the join handle anyway to cancel the task, unlock with nothing
            }
        }
        self.unlock(None);

        // Return the prior result if we have one
        Ok(out)
    }

    // Possibly spawn the future possibly returning the value of the last execution
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            pub async fn single_spawn(
                &self,
                future: impl Future<Output = T> + 'static,
            ) -> Result<Option<T>, ()> {
                let mut out: Option<T> = None;

                // See if we have a result we can return
                let maybe_jh = match self.try_lock() {
                    Ok(v) => v,
                    Err(_) => {
                        // If we are already polling somewhere else, don't hand back a result
                        return Err(());
                    }
                };
                let mut run = true;

                if maybe_jh.is_some() {
                    let mut jh = maybe_jh.unwrap();

                    // See if we finished, if so, return the value of the last execution
                    if let Poll::Ready(r) = poll!(&mut jh) {
                        out = Some(r);
                        // Task finished, unlock with a new task
                    } else {
                        // Still running, don't run again, unlock with the current join handle
                        run = false;
                        self.unlock(Some(jh));
                    }
                }

                // Run if we should do that
                if run {
                    self.unlock(Some(spawn_local(future)));
                }

                // Return the prior result if we have one
                Ok(out)
            }
        }
    }
}
cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        impl<T> SingleFuture<T>
        where
            T: 'static + Send,
        {
            pub async fn single_spawn(
                &self,
                future: impl Future<Output = T> + Send + 'static,
            ) -> Result<Option<T>, ()> {
                let mut out: Option<T> = None;
                // See if we have a result we can return
                let maybe_jh = match self.try_lock() {
                    Ok(v) => v,
                    Err(_) => {
                        // If we are already polling somewhere else, don't hand back a result
                        return Err(());
                    }
                };
                let mut run = true;
                if maybe_jh.is_some() {
                    let mut jh = maybe_jh.unwrap();
                    // See if we finished, if so, return the value of the last execution
                    if let Poll::Ready(r) = poll!(&mut jh) {
                        out = Some(r);
                        // Task finished, unlock with a new task
                    } else {
                        // Still running, don't run again, unlock with the current join handle
                        run = false;
                        self.unlock(Some(jh));
                    }
                }
                // Run if we should do that
                if run {
                    self.unlock(Some(spawn(future)));
                }
                // Return the prior result if we have one
                Ok(out)
            }
        }
    }
}
