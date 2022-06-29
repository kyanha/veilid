#![allow(dead_code)]

use crate::xx::*;
cfg_if! {
    if #[cfg(feature="rt-async-std")] {
        use async_std_resolver::{config, resolver, resolver_from_system_conf, AsyncStdResolver as AsyncResolver};
    } else if #[cfg(feature="rt-tokio")] {
        use trust_dns_resolver::{config, TokioAsyncResolver as AsyncResolver, error::ResolveError};

        pub async fn resolver(
            config: config::ResolverConfig,
            options: config::ResolverOpts,
        ) -> Result<AsyncResolver, ResolveError> {
            AsyncResolver::tokio(config, options)
        }

        /// Constructs a new async-std based Resolver with the system configuration.
        ///
        /// This will use `/etc/resolv.conf` on Unix OSes and the registry on Windows.
        #[cfg(any(unix, target_os = "windows"))]
        pub async fn resolver_from_system_conf() -> Result<AsyncResolver, ResolveError> {
            AsyncResolver::tokio_from_system_conf()
        }
    }
}
use rand::prelude::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

lazy_static::lazy_static! {
    static ref RESOLVER: Arc<AsyncMutex<Option<AsyncResolver>>> = Arc::new(AsyncMutex::new(None));
}

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
        cfg_if! {
            if #[cfg(feature="rt-async-std")] {
                async_std::task::yield_now().await;
            } else if #[cfg(feature="rt-tokio")] {
                tokio::task::yield_now().await;
            }
        }
    } else {
        cfg_if! {
            if #[cfg(feature="rt-async-std")] {
                async_std::task::sleep(Duration::from_millis(u64::from(millis))).await;
            } else if #[cfg(feature="rt-tokio")] {
                tokio::time::sleep(Duration::from_millis(u64::from(millis))).await;
            }
        }
    }
}

pub fn system_boxed<'a, Out>(
    future: impl Future<Output = Out> + Send + 'a,
) -> SystemPinBoxFutureLifetime<'a, Out> {
    Box::pin(future)
}

pub fn spawn<Out>(future: impl Future<Output = Out> + Send + 'static) -> MustJoinHandle<Out>
where
    Out: Send + 'static,
{
    cfg_if! {
        if #[cfg(feature="rt-async-std")] {
            MustJoinHandle::new(async_std::task::spawn(future))
        } else if #[cfg(feature="rt-tokio")] {
            MustJoinHandle::new(tokio::task::spawn(future))
        }
    }
}

pub fn spawn_local<Out>(future: impl Future<Output = Out> + 'static) -> MustJoinHandle<Out>
where
    Out: 'static,
{
    cfg_if! {
        if #[cfg(feature="rt-async-std")] {
            MustJoinHandle::new(async_std::task::spawn_local(future))
        } else if #[cfg(feature="rt-tokio")] {
            MustJoinHandle::new(tokio::task::spawn_local(future))
        }
    }
}

pub fn spawn_with_local_set<Out>(
    future: impl Future<Output = Out> + Send + 'static,
) -> MustJoinHandle<Out>
where
    Out: Send + 'static,
{
    cfg_if! {
        if #[cfg(feature="rt-async-std")] {
            spawn(future)
        } else if #[cfg(feature="rt-tokio")] {
            MustJoinHandle::new(tokio::task::spawn_blocking(move || {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let local = tokio::task::LocalSet::new();
                    local.run_until(future).await
                })
            }))
        }
    }
}

pub fn spawn_detached<Out>(future: impl Future<Output = Out> + Send + 'static)
where
    Out: Send + 'static,
{
    cfg_if! {
        if #[cfg(feature="rt-async-std")] {
            drop(async_std::task::spawn(future));
        } else if #[cfg(feature="rt-tokio")] {
            drop(tokio::task::spawn(future));
        }
    }
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

cfg_if! {
    if #[cfg(feature="rt-async-std")] {
        pub use async_std::future::TimeoutError;
    } else if #[cfg(feature="rt-tokio")] {
        pub use tokio::time::error::Elapsed as TimeoutError;
    }
}

pub async fn timeout<F, T>(dur_ms: u32, f: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    cfg_if! {
        if #[cfg(feature="rt-async-std")] {
            async_std::future::timeout(Duration::from_millis(dur_ms as u64), f).await
        } else if #[cfg(feature="rt-tokio")] {
            tokio::time::timeout(Duration::from_millis(dur_ms as u64), f).await
        }
    }
}

pub fn get_concurrency() -> u32 {
    std::thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or_else(|e| {
            warn!("unable to get concurrency defaulting to single core: {}", e);
            1
        }) as u32
}

pub async fn get_outbound_relay_peer() -> Option<crate::veilid_api::PeerInfo> {
    panic!("Native Veilid should never require an outbound relay");
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

async fn get_resolver() -> Result<AsyncResolver, String> {
    let mut resolver_lock = RESOLVER.lock().await;
    if let Some(r) = &*resolver_lock {
        Ok(r.clone())
    } else {
        let resolver = match resolver_from_system_conf().await {
            Ok(v) => v,
            Err(_) => resolver(
                config::ResolverConfig::default(),
                config::ResolverOpts::default(),
            )
            .await
            .expect("failed to connect resolver"),
        };

        *resolver_lock = Some(resolver.clone());
        Ok(resolver)
    }
}

pub async fn txt_lookup<S: AsRef<str>>(host: S) -> Result<Vec<String>, String> {
    let resolver = get_resolver().await?;
    let txt_result = resolver
        .txt_lookup(host.as_ref())
        .await
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for x in txt_result.iter() {
        for s in x.txt_data() {
            out.push(String::from_utf8(s.to_vec()).map_err(|e| e.to_string())?);
        }
    }
    Ok(out)
}

pub async fn ptr_lookup(ip_addr: IpAddr) -> Result<String, String> {
    let resolver = get_resolver().await?;
    let ptr_result = resolver
        .reverse_lookup(ip_addr)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(r) = ptr_result.iter().next() {
        Ok(r.to_string().trim_end_matches('.').to_string())
    } else {
        Err("PTR lookup returned an empty string".to_owned())
    }
}
