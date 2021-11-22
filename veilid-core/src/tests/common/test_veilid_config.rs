use crate::xx::*;
use crate::*;
cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::fs::File;
        use std::io::prelude::*;
        use std::path::PathBuf;

        static CERTFILE: &str = r#"-----BEGIN CERTIFICATE-----
MIIDbzCCAlegAwIBAgIRALB/PvRpqN55Pk7L33NNsvcwDQYJKoZIhvcNAQELBQAw
FDESMBAGA1UEAwwJTm9jdGVtIENBMB4XDTIwMDkwODIxMDkwMFoXDTMwMDkwNjIx
MDkwMFowHDEaMBgGA1UEAwwRKi5ub2N0ZW0uaW50ZXJuYWwwggEiMA0GCSqGSIb3
DQEBAQUAA4IBDwAwggEKAoIBAQDRbAtA2dIlTPaQUN43/bdGi2wuDzCXk36TcfOr
YoxGsyJV6QpcIdmtrPN2WbkuDmA/G+0BUcQPvBfA/pFRHQElrzMhGR23Mp6IK7YR
pomUa1DQSJyMw/WM9V0+tidp5tJSeUCB+qKhLBrztD5XXjdhU6WA1J0y26XQoBqs
RZbPV8mce4LxVaQptkf4NB4/jnr3M1/FWEri60xBw3blWGaLP6gza3vqAr8pqEY4
zXU4q+egLbRIOwxwBJ0/vcyO6BdSzA1asWJCddXQJkUQrLl3OQ+44FMsAFyzCOiK
DVoqD2z4IJvIRT6TH8OcYvrotytlsNXS4ja9r32tTR1/DxUrAgMBAAGjgbMwgbAw
CQYDVR0TBAIwADAdBgNVHQ4EFgQUhjP4CArB3wWGHfavf7mRxaYshKMwRAYDVR0j
BD0wO4AUKAOv10AaiIUHgOtx0Mk6ZaZ/tGWhGKQWMBQxEjAQBgNVBAMMCU5vY3Rl
bSBDQYIJAISVWafozd3RMBMGA1UdJQQMMAoGCCsGAQUFBwMBMAsGA1UdDwQEAwIF
oDAcBgNVHREEFTATghEqLm5vY3RlbS5pbnRlcm5hbDANBgkqhkiG9w0BAQsFAAOC
AQEAMfVGtpXdkxflSQY2DzIUXLp9cZQnu4A8gww8iaLAg5CIUijP71tb2JJ+SsRx
W3p14YMhOYtswIvGTtXWzMgfAivwrxCcJefnqDAG9yviWoA0CSQe21nRjEqN6nyh
CS2BIkOcNNf10TD9sNo7z6IIXNjok7/F031JvH6pBgZ8Bq4IE/ANIuAvxwslPrqT
80qnWtAc5TzNNR1CT+fyZwMEpeW5fMZQnrSyUMsNv06Jydl/7IkGvlmbwihZOg95
Vty37pyzrXU5s/DY1zi5aYoFiK7/4bNEy9mRL9ero+kCvQfea0Yt2rITKQkCYvKu
MQTNaSyo6GTifW5InckkQIsnTQ==
-----END CERTIFICATE-----"#;

        static KEYFILE: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDRbAtA2dIlTPaQ
UN43/bdGi2wuDzCXk36TcfOrYoxGsyJV6QpcIdmtrPN2WbkuDmA/G+0BUcQPvBfA
/pFRHQElrzMhGR23Mp6IK7YRpomUa1DQSJyMw/WM9V0+tidp5tJSeUCB+qKhLBrz
tD5XXjdhU6WA1J0y26XQoBqsRZbPV8mce4LxVaQptkf4NB4/jnr3M1/FWEri60xB
w3blWGaLP6gza3vqAr8pqEY4zXU4q+egLbRIOwxwBJ0/vcyO6BdSzA1asWJCddXQ
JkUQrLl3OQ+44FMsAFyzCOiKDVoqD2z4IJvIRT6TH8OcYvrotytlsNXS4ja9r32t
TR1/DxUrAgMBAAECggEBAMIAK+CUqCbjyBliwKjvwWN5buqwKZyRBxXB3y/qJ/aq
pWkea/lzZjqMWDFP5sryiFiOHx00yMKmxP6FFMsmalSlm2DS6oM2QkP08kIhm5vB
WmjIizWfpo5BEnMwvQxOxpGeP5LpQtS5jfIrDAFVh0oC+fOBgmqFrXK5jlv+Tzmc
9PzoF5lgy8CHw3NxuScJpEhA1vTzu5N7sTdiTDKqY1ph2+RFlf30oyx4whoRVpIC
w8vp3WbLu/yAGuN5S14mYJW2Qgi8/rVCDStROEKOeB99mt1MG5lX7iuagzS/95Lr
2m1Nya0+7hkkpq6Y3Wqne9H0NLasJK8PU8ZaEc6BwTkCgYEA8iLVBrt4W/Cc5hry
8LWCMX8P25z7WIRYswnPvqwTwE0f6Q1ddWIaR9GPWUHgoRC4Z0b0MKolwo9s8RPE
GBuTOCy8ArSgYb1jNpsanGIWg6mZZgfylKdMdCMXMAAYF1/sTXeqCDY+FSCzEAvZ
hzppcCpiKV7Pa9aOo7o3/IeUBZcCgYEA3WmyvscG27R18XASJYL8Y4DuFvvnTHMp
YnxJIoS1+0TnUD2QqXUnXKbnTioWs7t990YAjbsHvK4fVsbnkuEm/as0oYbC8vU1
W3XN0HrpiacGcYIcXU4AY4XvY8t3y76FycJAT9Q6QztVofI5DmXV+8qsyrEegUys
wPIkkumCJ40CgYBKT3hTPZudk8WDNQgT6ZCQQi+Kta3Jp6xVHhC8srDJFqJRcsGY
8ceg/OZifT5EEA6X24W7naxC/qNvhSJsR6Ix3kDBD9AczvOw4X8UOWIxfA5Q6uV+
y61CAzbti0nZep3Z1HzBUmxRLZzmssxKnRmYy9keWzOLI+jYxKDEBpPd9wKBgAY1
pquvDUQwJXal+/xNViK8RPEkE3KTcD+w2KQ9MJVhc1NOxrXZ8Uap76bDi2tzAK9k
qTNQYYErKPnYDjqSUfOfT5SQIPuLYPm1rhYAvHf91TJtwbnkLCKeaP5VgICYUUw9
RGx4uUGVcmteTbdXp86t+naczQw3SEkJAXmVTu8pAoGATF7xXifMUSL1v43Ybrmc
RikQyDecRspMYLOCNmPWI2PPz6MAjm8jDCsXK52HUK4mUqrd/W3rqnl+TrJsXOnH
Ww6tESPaF1kCVyV2Jx/5m8qsE9y5Bds7eMo2JF8vnAKFX6t4KwZiyHBymj6uelNc
wFAbkZY9eS/x6P7qrpd7dUA=
-----END PRIVATE KEY-----"#;
    }
}

cfg_if! {

    if #[cfg(target_arch = "wasm32")] {
        pub fn get_tablestore_path() -> String {
            String::new()
        }
        pub fn get_certfile_path() -> String {
            String::new()
        }
        pub fn get_keyfile_path() -> String {
            String::new()
        }
    }
    else {

        fn get_data_dir() -> PathBuf {
            let out;
            cfg_if! {
                if #[cfg(target_os = "android")] {
                    out = PathBuf::from(intf::utils::android::get_files_dir());
                } else {
                    use directories::*;

                    if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "VeilidCoreTests") {
                        out = PathBuf::from(my_proj_dirs.data_local_dir());
                    } else {
                        out = PathBuf::from("./");
                    }
                }
            }
            out
        }

        pub fn get_tablestore_path() -> String {
            let mut out = get_data_dir();
            std::fs::create_dir_all(&out).unwrap();

            out.push("tablestore");

            out.into_os_string().into_string().unwrap()
        }

        pub fn get_certfile_path() -> String {
            let mut out = get_data_dir();
            std::fs::create_dir_all(&out).unwrap();

            out.push("cert.pem");
            // Initialize certfile
            if !out.exists() {
                debug!("creating certfile at {:?}", out);
                File::create(&out).unwrap().write_all(CERTFILE.as_bytes()).unwrap();
            }

            out.into_os_string().into_string().unwrap()
        }

        pub fn get_keyfile_path() -> String {
            let mut out = get_data_dir();
            std::fs::create_dir_all(&out).unwrap();

            out.push("key.pem");

            // Initialize keyfile
            if !out.exists() {
                debug!("creating keyfile at {:?}", out);
                File::create(&out).unwrap().write_all(KEYFILE.as_bytes()).unwrap();
            }

            out.into_os_string().into_string().unwrap()
        }
    }
}

pub fn setup_veilid_core() -> VeilidCoreSetup {
    VeilidCoreSetup {
        state_change_callback: Arc::new(
            move |change: VeilidStateChange| -> SystemPinBoxFuture<()> {
                Box::pin(async move {
                    trace!("state_change_callback: {:?}", change);
                })
            },
        ),
        config_callback: Arc::new(config_callback),
    }
}

pub fn config_callback(key: String) -> Result<Box<dyn core::any::Any>, String> {
    match key.as_str() {
        "namespace" => Ok(Box::new(String::from(""))),
        "capabilities.protocol_udp" => Ok(Box::new(true)),
        "capabilities.protocol_connect_tcp" => Ok(Box::new(true)),
        "capabilities.protocol_accept_tcp" => Ok(Box::new(true)),
        "capabilities.protocol_connect_ws" => Ok(Box::new(true)),
        "capabilities.protocol_accept_ws" => Ok(Box::new(true)),
        "capabilities.protocol_connect_wss" => Ok(Box::new(true)),
        "capabilities.protocol_accept_wss" => Ok(Box::new(true)),
        "tablestore.directory" => Ok(Box::new(get_tablestore_path())),
        "network.max_connections" => Ok(Box::new(16u32)),
        "network.connection_initial_timeout" => Ok(Box::new(2_000_000u64)),
        "network.node_id" => Ok(Box::new(dht::key::DHTKey::default())),
        "network.node_id_secret" => Ok(Box::new(dht::key::DHTKeySecret::default())),
        "network.bootstrap" => Ok(Box::new(vec![String::from("asdf"), String::from("qwer")])),
        "network.rpc.concurrency" => Ok(Box::new(2u32)),
        "network.rpc.queue_size" => Ok(Box::new(128u32)),
        "network.rpc.max_timestamp_behind" => Ok(Box::new(Some(10_000_000u64))),
        "network.rpc.max_timestamp_ahead" => Ok(Box::new(Some(10_000_000u64))),
        "network.rpc.timeout" => Ok(Box::new(10_000_000u64)),
        "network.rpc.max_route_hop_count" => Ok(Box::new(7u8)),
        "network.dht.resolve_node_timeout" => Ok(Box::new(Option::<u64>::None)),
        "network.dht.resolve_node_count" => Ok(Box::new(20u32)),
        "network.dht.resolve_node_fanout" => Ok(Box::new(3u32)),
        "network.dht.max_find_node_count" => Ok(Box::new(20u32)),
        "network.dht.get_value_timeout" => Ok(Box::new(Option::<u64>::None)),
        "network.dht.get_value_count" => Ok(Box::new(20u32)),
        "network.dht.get_value_fanout" => Ok(Box::new(3u32)),
        "network.dht.set_value_timeout" => Ok(Box::new(Option::<u64>::None)),
        "network.dht.set_value_count" => Ok(Box::new(20u32)),
        "network.dht.set_value_fanout" => Ok(Box::new(5u32)),
        "network.dht.min_peer_count" => Ok(Box::new(20u32)),
        "network.dht.min_peer_refresh_time" => Ok(Box::new(2000000u64)),
        "network.dht.validate_dial_info_receipt_time" => Ok(Box::new(5000000u64)),
        "network.upnp" => Ok(Box::new(false)),
        "network.natpmp" => Ok(Box::new(false)),
        "network.address_filter" => Ok(Box::new(true)),
        "network.tls.certificate_path" => Ok(Box::new(get_certfile_path())),
        "network.tls.private_key_path" => Ok(Box::new(get_keyfile_path())),
        "network.tls.connection_initial_timeout" => Ok(Box::new(2_000_000u64)),
        "network.application.path" => Ok(Box::new(String::from("/app"))),
        "network.application.https.enabled" => Ok(Box::new(true)),
        "network.application.https.listen_address" => Ok(Box::new(String::from("[::1]:5150"))),
        "network.application.http.enabled" => Ok(Box::new(true)),
        "network.application.http.listen_address" => Ok(Box::new(String::from("[::1]:5150"))),
        "network.protocol.udp.enabled" => Ok(Box::new(true)),
        "network.protocol.udp.socket_pool_size" => Ok(Box::new(0u32)),
        "network.protocol.udp.listen_address" => Ok(Box::new(String::from("[::1]:5150"))),
        "network.protocol.udp.public_address" => Ok(Box::new(Option::<String>::None)),
        "network.protocol.tcp.connect" => Ok(Box::new(true)),
        "network.protocol.tcp.listen" => Ok(Box::new(true)),
        "network.protocol.tcp.max_connections" => Ok(Box::new(32u32)),
        "network.protocol.tcp.listen_address" => Ok(Box::new(String::from("[::1]:5150"))),
        "network.protocol.tcp.public_address" => Ok(Box::new(Option::<String>::None)),
        "network.protocol.ws.connect" => Ok(Box::new(true)),
        "network.protocol.ws.listen" => Ok(Box::new(true)),
        "network.protocol.ws.max_connections" => Ok(Box::new(16u32)),
        "network.protocol.ws.listen_address" => Ok(Box::new(String::from("[::1]:5150"))),
        "network.protocol.ws.path" => Ok(Box::new(String::from("/ws"))),
        "network.protocol.ws.public_address" => Ok(Box::new(Option::<String>::None)),
        "network.protocol.wss.connect" => Ok(Box::new(true)),
        "network.protocol.wss.listen" => Ok(Box::new(true)),
        "network.protocol.wss.max_connections" => Ok(Box::new(16u32)),
        "network.protocol.wss.listen_address" => Ok(Box::new(String::from("[::1]:5150"))),
        "network.protocol.wss.path" => Ok(Box::new(String::from("/ws"))),
        "network.protocol.wss.public_address" => Ok(Box::new(Option::<String>::None)),
        "network.leases.max_server_signal_leases" => Ok(Box::new(256u32)),
        "network.leases.max_server_relay_leases" => Ok(Box::new(8u32)),
        "network.leases.max_client_signal_leases" => Ok(Box::new(2u32)),
        "network.leases.max_client_relay_leases" => Ok(Box::new(2u32)),
        _ => Err(format!("config key '{}' doesn't exist", key)),
    }
}

pub async fn test_config() {
    let mut vc = VeilidConfig::new();
    match vc.init(Arc::new(config_callback)).await {
        Ok(()) => (),
        Err(e) => {
            error!("Error: {}", e);
            assert!(false);
        }
    }
    let inner = vc.get();
    assert_eq!(inner.namespace, String::from(""));
    assert_eq!(inner.capabilities.protocol_udp, true);
    assert_eq!(inner.capabilities.protocol_connect_tcp, true);
    assert_eq!(inner.capabilities.protocol_accept_tcp, true);
    assert_eq!(inner.capabilities.protocol_connect_ws, true);
    assert_eq!(inner.capabilities.protocol_accept_ws, true);
    assert_eq!(inner.capabilities.protocol_connect_wss, true);
    assert_eq!(inner.capabilities.protocol_accept_wss, true);
    assert_eq!(inner.tablestore.directory, get_tablestore_path());
    assert_eq!(inner.network.max_connections, 16);
    assert_eq!(inner.network.connection_initial_timeout, 2_000_000u64);
    assert!(inner.network.node_id.valid);
    assert!(inner.network.node_id_secret.valid);
    assert_eq!(
        inner.network.bootstrap,
        vec![String::from("asdf"), String::from("qwer")]
    );
    assert_eq!(inner.network.rpc.concurrency, 2u32);
    assert_eq!(inner.network.rpc.queue_size, 128u32);
    assert_eq!(inner.network.rpc.timeout, 10_000_000u64);
    assert_eq!(inner.network.rpc.max_route_hop_count, 7u8);
    assert_eq!(inner.network.dht.resolve_node_timeout, Option::<u64>::None);
    assert_eq!(inner.network.dht.resolve_node_count, 20u32);
    assert_eq!(inner.network.dht.resolve_node_fanout, 3u32);
    assert_eq!(inner.network.dht.get_value_timeout, Option::<u64>::None);
    assert_eq!(inner.network.dht.get_value_count, 20u32);
    assert_eq!(inner.network.dht.get_value_fanout, 3u32);
    assert_eq!(inner.network.dht.set_value_timeout, Option::<u64>::None);
    assert_eq!(inner.network.dht.set_value_count, 20u32);
    assert_eq!(inner.network.dht.set_value_fanout, 5u32);
    assert_eq!(inner.network.dht.min_peer_count, 20u32);
    assert_eq!(inner.network.dht.min_peer_refresh_time, 2000000u64);
    assert_eq!(
        inner.network.dht.validate_dial_info_receipt_time,
        5000000u64
    );

    assert_eq!(inner.network.upnp, false);
    assert_eq!(inner.network.natpmp, false);
    assert_eq!(inner.network.address_filter, true);
    assert_eq!(inner.network.tls.certificate_path, get_certfile_path());
    assert_eq!(inner.network.tls.private_key_path, get_keyfile_path());
    assert_eq!(inner.network.tls.connection_initial_timeout, 2_000_000u64);

    assert_eq!(inner.network.application.path, "/app");
    assert_eq!(inner.network.application.https.enabled, true);
    assert_eq!(inner.network.application.https.listen_address, "[::1]:5150");
    assert_eq!(inner.network.application.http.enabled, true);
    assert_eq!(inner.network.application.http.listen_address, "[::1]:5150");
    assert_eq!(inner.network.protocol.udp.enabled, true);
    assert_eq!(inner.network.protocol.udp.socket_pool_size, 0u32);
    assert_eq!(inner.network.protocol.udp.listen_address, "[::1]:5150");
    assert_eq!(inner.network.protocol.udp.public_address, None);
    assert_eq!(inner.network.protocol.tcp.connect, true);
    assert_eq!(inner.network.protocol.tcp.listen, true);
    assert_eq!(inner.network.protocol.tcp.max_connections, 32u32);
    assert_eq!(inner.network.protocol.tcp.listen_address, "[::1]:5150");
    assert_eq!(inner.network.protocol.tcp.public_address, None);
    assert_eq!(inner.network.protocol.ws.connect, true);
    assert_eq!(inner.network.protocol.ws.listen, true);
    assert_eq!(inner.network.protocol.ws.max_connections, 16u32);
    assert_eq!(inner.network.protocol.ws.listen_address, "[::1]:5150");
    assert_eq!(inner.network.protocol.ws.path, "/ws");
    assert_eq!(inner.network.protocol.ws.public_address, None);
    assert_eq!(inner.network.protocol.wss.connect, true);
    assert_eq!(inner.network.protocol.wss.listen, true);
    assert_eq!(inner.network.protocol.wss.max_connections, 16u32);
    assert_eq!(inner.network.protocol.wss.listen_address, "[::1]:5150");
    assert_eq!(inner.network.protocol.wss.path, "/ws");
    assert_eq!(inner.network.protocol.wss.public_address, None);
}

pub async fn test_all() {
    test_config().await;
}
