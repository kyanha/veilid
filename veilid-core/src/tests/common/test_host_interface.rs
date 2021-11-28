use crate::xx::*;
use crate::*;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use js_sys::*;
    } else {
        use std::time::{Duration, SystemTime};
    }
}

pub async fn test_log() {
    info!("testing log");
}

pub async fn test_get_timestamp() {
    info!("testing get_timestamp");
    let t1 = intf::get_timestamp();
    let t2 = intf::get_timestamp();
    assert!(t2 >= t1);
}

pub async fn test_eventual() {
    info!("testing Eventual");
    {
        let e1 = Eventual::new();
        let i1 = e1.instance_clone(1u32);
        let i2 = e1.instance_clone(2u32);
        let i3 = e1.instance_clone(3u32);
        drop(i3);
        let i4 = e1.instance_clone(4u32);
        drop(i2);

        let jh = intf::spawn(async move {
            intf::sleep(1000).await;
            e1.resolve();
        });

        assert_eq!(i1.await, 1u32);
        assert_eq!(i4.await, 4u32);

        jh.await;
    }
    {
        let e1 = Eventual::new();
        let i1 = e1.instance_clone(1u32);
        let i2 = e1.instance_clone(2u32);
        let i3 = e1.instance_clone(3u32);
        let i4 = e1.instance_clone(4u32);
        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            let i5 = e1.instance_clone(5u32);
            let i6 = e1.instance_clone(6u32);
            assert_eq!(i1.await, 1u32);
            assert_eq!(i5.await, 5u32);
            assert_eq!(i6.await, 6u32);
        });
        intf::sleep(1000).await;
        let resolved = e1_c1.resolve();
        drop(i2);
        drop(i3);
        assert_eq!(i4.await, 4u32);
        resolved.await;
        jh.await;
    }
    {
        let e1 = Eventual::new();
        let i1 = e1.instance_clone(1u32);
        let i2 = e1.instance_clone(2u32);
        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            assert_eq!(i1.await, 1u32);
            assert_eq!(i2.await, 2u32);
        });
        intf::sleep(1000).await;
        e1_c1.resolve().await;

        jh.await;

        e1_c1.reset();
        //
        let j1 = e1.instance_clone(1u32);
        let j2 = e1.instance_clone(2u32);
        let jh = intf::spawn(async move {
            assert_eq!(j1.await, 1u32);
            assert_eq!(j2.await, 2u32);
        });
        intf::sleep(1000).await;
        e1_c1.resolve().await;

        jh.await;

        e1_c1.reset();
    }
}

pub async fn test_eventual_value() {
    info!("testing Eventual Value");
    {
        let e1 = EventualValue::<u32>::new();
        let i1 = e1.instance();
        let i2 = e1.instance();
        let i3 = e1.instance();
        drop(i3);
        let i4 = e1.instance();
        drop(i2);

        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            intf::sleep(1000).await;
            e1_c1.resolve(3u32);
        });

        i1.await;
        i4.await;
        jh.await;
        assert_eq!(e1.take_value(), Some(3u32));
    }
    {
        let e1 = EventualValue::new();
        let i1 = e1.instance();
        let i2 = e1.instance();
        let i3 = e1.instance();
        let i4 = e1.instance();
        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            let i5 = e1.instance();
            let i6 = e1.instance();
            i1.await;
            i5.await;
            i6.await;
        });
        intf::sleep(1000).await;
        let resolved = e1_c1.resolve(4u16);
        drop(i2);
        drop(i3);
        i4.await;
        resolved.await;
        jh.await;
        assert_eq!(e1_c1.take_value(), Some(4u16));
    }
    {
        let e1 = EventualValue::new();
        assert_eq!(e1.take_value(), None);
        let i1 = e1.instance();
        let i2 = e1.instance();
        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            i1.await;
            i2.await;
        });
        intf::sleep(1000).await;
        e1_c1.resolve(5u32).await;
        jh.await;
        assert_eq!(e1_c1.take_value(), Some(5u32));
        e1_c1.reset();
        assert_eq!(e1_c1.take_value(), None);
        //
        let j1 = e1.instance();
        let j2 = e1.instance();
        let jh = intf::spawn(async move {
            j1.await;
            j2.await;
        });
        intf::sleep(1000).await;
        e1_c1.resolve(6u32).await;
        jh.await;
        assert_eq!(e1_c1.take_value(), Some(6u32));
        e1_c1.reset();
        assert_eq!(e1_c1.take_value(), None);
    }
}

pub async fn test_eventual_value_clone() {
    info!("testing Eventual Value Clone");
    {
        let e1 = EventualValueClone::<u32>::new();
        let i1 = e1.instance();
        let i2 = e1.instance();
        let i3 = e1.instance();
        drop(i3);
        let i4 = e1.instance();
        drop(i2);

        let jh = intf::spawn(async move {
            intf::sleep(1000).await;
            e1.resolve(3u32);
        });

        assert_eq!(i1.await, 3);
        assert_eq!(i4.await, 3);

        jh.await;
    }

    {
        let e1 = EventualValueClone::new();
        let i1 = e1.instance();
        let i2 = e1.instance();
        let i3 = e1.instance();
        let i4 = e1.instance();
        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            let i5 = e1.instance();
            let i6 = e1.instance();
            assert_eq!(i1.await, 4);
            assert_eq!(i5.await, 4);
            assert_eq!(i6.await, 4);
        });
        intf::sleep(1000).await;
        let resolved = e1_c1.resolve(4u16);
        drop(i2);
        drop(i3);
        assert_eq!(i4.await, 4);
        resolved.await;
        jh.await;
    }

    {
        let e1 = EventualValueClone::new();
        let i1 = e1.instance();
        let i2 = e1.instance();
        let e1_c1 = e1.clone();
        let jh = intf::spawn(async move {
            assert_eq!(i1.await, 5);
            assert_eq!(i2.await, 5);
        });
        intf::sleep(1000).await;
        e1_c1.resolve(5u32).await;
        jh.await;
        e1_c1.reset();
        //
        let j1 = e1.instance();
        let j2 = e1.instance();
        let jh = intf::spawn(async move {
            assert_eq!(j1.await, 6);
            assert_eq!(j2.await, 6);
        });
        intf::sleep(1000).await;
        e1_c1.resolve(6u32).await;
        jh.await;
        e1_c1.reset();
    }
}
pub async fn test_interval() {
    info!("testing interval");

    let tick: Arc<Mutex<u32>> = Arc::new(Mutex::new(0u32));
    let stopper = intf::interval(1000, move || {
        let tick = tick.clone();
        async move {
            let mut tick = tick.lock();
            trace!("tick {}", tick);
            *tick += 1;
        }
    });

    intf::sleep(5500).await;

    stopper.await;
}

pub async fn test_timeout() {
    info!("testing timeout");

    let tick: Arc<Mutex<u32>> = Arc::new(Mutex::new(0u32));
    let tick_1 = tick.clone();
    assert!(
        intf::timeout(2500, async move {
            let mut tick = tick_1.lock();
            trace!("tick {}", tick);
            intf::sleep(1000).await;
            *tick += 1;
            trace!("tick {}", tick);
            intf::sleep(1000).await;
            *tick += 1;
            trace!("tick {}", tick);
            intf::sleep(1000).await;
            *tick += 1;
            trace!("tick {}", tick);
            intf::sleep(1000).await;
            *tick += 1;
        })
        .await
        .is_err(),
        "should have timed out"
    );

    let ticks = *tick.lock();
    assert!(ticks <= 2);
}

pub async fn test_sleep() {
    info!("testing sleep");
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {

            let t1 = Date::now();
            intf::sleep(1000).await;
            let t2 = Date::now();
            assert!((t2-t1) >= 1000.0);

        } else {

            let sys_time = SystemTime::now();
            let one_sec = Duration::from_secs(1);

            intf::sleep(1000).await;
            assert!(sys_time.elapsed().unwrap() >= one_sec);
        }
    }
}

pub async fn test_protected_store() {
    info!("testing protected store");

    let _ = intf::remove_user_secret("test", "_test_key").await;
    let _ = intf::remove_user_secret("test", "_test_broken").await;

    let d1: [u8; 0] = [];

    assert_eq!(
        intf::save_user_secret("test", "_test_key", &[2u8, 3u8, 4u8]).await,
        Ok(false)
    );
    info!("testing saving user secret");
    assert_eq!(
        intf::save_user_secret("test", "_test_key", &d1).await,
        Ok(true)
    );
    info!("testing loading user secret");
    assert_eq!(
        intf::load_user_secret("test", "_test_key").await,
        Ok(Some(d1.to_vec()))
    );
    info!("testing loading user secret again");
    assert_eq!(
        intf::load_user_secret("test", "_test_key").await,
        Ok(Some(d1.to_vec()))
    );
    info!("testing loading broken user secret");
    assert_eq!(
        intf::load_user_secret("test", "_test_broken").await,
        Ok(None)
    );
    info!("testing loading broken user secret again");
    assert_eq!(
        intf::load_user_secret("test", "_test_broken").await,
        Ok(None)
    );
    info!("testing remove user secret");
    assert_eq!(
        intf::remove_user_secret("test", "_test_key").await,
        Ok(true)
    );
    info!("testing remove user secret again");
    assert_eq!(
        intf::remove_user_secret("test", "_test_key").await,
        Ok(false)
    );
    info!("testing remove broken user secret");
    assert_eq!(
        intf::remove_user_secret("test", "_test_broken").await,
        Ok(false)
    );
    info!("testing remove broken user secret again");
    assert_eq!(
        intf::remove_user_secret("test", "_test_broken").await,
        Ok(false)
    );

    let d2: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    assert_eq!(
        intf::save_user_secret("test", "_test_key", &[2u8, 3u8, 4u8]).await,
        Ok(false)
    );
    assert_eq!(
        intf::save_user_secret("test", "_test_key", &d2).await,
        Ok(true)
    );
    assert_eq!(
        intf::load_user_secret("test", "_test_key").await,
        Ok(Some(d2.to_vec()))
    );
    assert_eq!(
        intf::load_user_secret("test", "_test_key").await,
        Ok(Some(d2.to_vec()))
    );
    assert_eq!(
        intf::load_user_secret("test", "_test_broken").await,
        Ok(None)
    );
    assert_eq!(
        intf::load_user_secret("test", "_test_broken").await,
        Ok(None)
    );
    assert_eq!(
        intf::remove_user_secret("test", "_test_key").await,
        Ok(true)
    );
    assert_eq!(
        intf::remove_user_secret("test", "_test_key").await,
        Ok(false)
    );
    assert_eq!(
        intf::remove_user_secret("test", "_test_broken").await,
        Ok(false)
    );
    assert_eq!(
        intf::remove_user_secret("test", "_test_broken").await,
        Ok(false)
    );

    let _ = intf::remove_user_secret("test", "_test_key").await;
    let _ = intf::remove_user_secret("test", "_test_broken").await;
}

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        pub async fn test_network_interfaces() {
            info!("testing network interfaces");
            let t1 = intf::get_timestamp();
            let mut interfaces = intf::utils::network_interfaces::NetworkInterfaces::new();
            let count = 100;
            for _ in 0..count {
                if let Err(e) = interfaces.refresh() {
                    error!("error refreshing interfaces: {}", e);
                }
            }
            let t2 = intf::get_timestamp();
            let tdiff = ((t2 - t1) as f64)/1000000.0f64;
            info!("running network interface test with {} iterations took {} seconds", count, tdiff);
        }
    }
}

pub async fn test_get_random_u64() {
    info!("testing random number generator for u64");
    let t1 = intf::get_timestamp();
    let count = 10000;
    for _ in 0..count {
        let _ = intf::get_random_u64();
    }
    let t2 = intf::get_timestamp();
    let tdiff = ((t2 - t1) as f64) / 1000000.0f64;
    info!(
        "running network interface test with {} iterations took {} seconds",
        count, tdiff
    );
}

pub async fn test_get_random_u32() {
    info!("testing random number generator for u32");
    let t1 = intf::get_timestamp();
    let count = 10000;
    for _ in 0..count {
        let _ = intf::get_random_u32();
    }
    let t2 = intf::get_timestamp();
    let tdiff = ((t2 - t1) as f64) / 1000000.0f64;
    info!(
        "running network interface test with {} iterations took {} seconds",
        count, tdiff
    );
}

pub async fn test_single_future() {
    info!("testing single future");
    let sf = SingleFuture::<u32>::new();
    assert_eq!(sf.check().await, Ok(None));
    assert_eq!(
        sf.single_spawn(async {
            intf::sleep(2000).await;
            69
        })
        .await,
        Ok(None)
    );
    assert_eq!(sf.check().await, Ok(None));
    assert_eq!(sf.single_spawn(async { panic!() }).await, Ok(None));
    assert_eq!(sf.join().await, Ok(Some(69)));
    assert_eq!(
        sf.single_spawn(async {
            intf::sleep(1000).await;
            37
        })
        .await,
        Ok(None)
    );
    intf::sleep(2000).await;
    assert_eq!(
        sf.single_spawn(async {
            intf::sleep(1000).await;
            27
        })
        .await,
        Ok(Some(37))
    );
    intf::sleep(2000).await;
    assert_eq!(sf.join().await, Ok(Some(27)));
    assert_eq!(sf.check().await, Ok(None));
}

pub async fn test_tools() {
    info!("testing retry_falloff_log");
    let mut last_us = 0u64;
    for x in 0..1024 {
        let cur_us = x as u64 * 1000000u64;
        if retry_falloff_log(last_us, cur_us, 10_000_000u64, 6_000_000_000u64, 2.0f64) {
            info!("   retry at {} secs", timestamp_to_secs(cur_us));
            last_us = cur_us;
        }
    }
}

pub async fn test_all() {
    test_log().await;
    test_get_timestamp().await;
    test_tools().await;
    test_get_random_u64().await;
    test_get_random_u32().await;
    test_sleep().await;
    #[cfg(not(target_arch = "wasm32"))]
    test_network_interfaces().await;
    test_single_future().await;
    test_eventual().await;
    test_eventual_value().await;
    test_eventual_value_clone().await;
    test_interval().await;
    test_timeout().await;
    test_protected_store().await;
}
