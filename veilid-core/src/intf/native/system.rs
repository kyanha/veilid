use crate::xx::*;
pub use async_executors::JoinHandle;
use async_executors::{AsyncStd, LocalSpawnHandleExt, SpawnHandleExt};
use rand::prelude::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn get_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n.as_micros() as u64,
        Err(_) => panic!("SystemTime before UNIX_EPOCH!"),
    }
}

pub fn get_timestamp_string() -> String {
    let dt = chrono::Utc::now();
    dt.time().format("%H:%M:%S.3f").to_string()
}

pub fn random_bytes(dest: &mut [u8]) -> Result<(), String> {
    let mut rng = rand::thread_rng();
    rng.try_fill_bytes(dest).map_err(|err| format!("{:?}", err))
}

pub fn get_random_u32() -> u32 {
    let mut rng = rand::thread_rng();
    rng.next_u32()
}

pub fn get_random_u64() -> u64 {
    let mut rng = rand::thread_rng();
    rng.next_u64()
}

pub async fn sleep(millis: u32) {
    if millis == 0 {
        async_std::task::yield_now().await;
    } else {
        async_std::task::sleep(Duration::from_millis(u64::from(millis))).await;
    }
}

pub fn spawn<Out>(future: impl Future<Output = Out> + Send + 'static) -> JoinHandle<Out>
where
    Out: Send + 'static,
{
    AsyncStd
        .spawn_handle(future)
        .expect("async-std spawn should never error out")
}

pub fn spawn_local<Out>(future: impl Future<Output = Out> + 'static) -> JoinHandle<Out>
where
    Out: 'static,
{
    AsyncStd
        .spawn_handle_local(future)
        .expect("async-std spawn_local should never error out")
}

pub fn interval<F, FUT>(freq_ms: u32, callback: F) -> SystemPinBoxFuture<()>
where
    F: Fn() -> FUT + Send + Sync + 'static,
    FUT: Future<Output = ()> + Send,
{
    let e = Eventual::new();

    let ie = e.clone();
    let jh = spawn(async move {
        while timeout(freq_ms, ie.instance_clone(())).await.is_err() {
            callback().await;
        }
    });

    Box::pin(async move {
        e.resolve().await;
        jh.await;
    })
}

pub use async_std::future::TimeoutError;

pub async fn timeout<F, T>(dur_ms: u32, f: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    async_std::future::timeout(Duration::from_millis(dur_ms as u64), f).await
}

pub fn get_concurrency() -> u32 {
    num_cpus::get() as u32
}

/*
pub fn async_callback<F, OF, EF, T, E>(fut: F, ok_fn: OF, err_fn: EF)
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    OF: FnOnce(T) + Send + 'static,
    EF: FnOnce(E) + Send + 'static,
{
    spawn(Box::pin(async move {
        match fut.await {
            Ok(v) => ok_fn(v),
            Err(e) => err_fn(e),
        };
    }));
}
*/
