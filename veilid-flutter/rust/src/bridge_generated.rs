#![allow(
    non_camel_case_types,
    unused,
    clippy::redundant_closure,
    clippy::useless_conversion,
    non_snake_case
)]
// AUTO GENERATED FILE, DO NOT EDIT.
// Generated by `flutter_rust_bridge`.

use crate::api::*;
use flutter_rust_bridge::*;

// Section: wire functions

#[no_mangle]
pub extern "C" fn wire_startup_veilid_core(port_: i64, config: *mut wire_VeilidConfig) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "startup_veilid_core",
            port: Some(port_),
            mode: FfiCallMode::Stream,
        },
        move || {
            let api_config = config.wire2api();
            move |task_callback| startup_veilid_core(task_callback.stream_sink(), api_config)
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_get_veilid_state(port_: i64) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "get_veilid_state",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| get_veilid_state(),
    )
}

#[no_mangle]
pub extern "C" fn wire_shutdown_veilid_core(port_: i64) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "shutdown_veilid_core",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| shutdown_veilid_core(),
    )
}

#[no_mangle]
pub extern "C" fn wire_veilid_version_string(port_: i64) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "veilid_version_string",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| veilid_version_string(),
    )
}

#[no_mangle]
pub extern "C" fn wire_veilid_version(port_: i64) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "veilid_version",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| veilid_version(),
    )
}

// Section: wire structs

#[repr(C)]
#[derive(Clone)]
pub struct wire_StringList {
    ptr: *mut *mut wire_uint_8_list,
    len: i32,
}

#[repr(C)]
#[derive(Clone)]
pub struct wire_uint_8_list {
    ptr: *mut u8,
    len: i32,
}

#[repr(C)]
#[derive(Clone)]
pub struct wire_VeilidConfig {
    program_name: *mut wire_uint_8_list,
    namespace: *mut wire_uint_8_list,
    capabilities__protocol_udp: bool,
    capabilities__protocol_connect_tcp: bool,
    capabilities__protocol_accept_tcp: bool,
    capabilities__protocol_connect_ws: bool,
    capabilities__protocol_accept_ws: bool,
    capabilities__protocol_connect_wss: bool,
    capabilities__protocol_accept_wss: bool,
    protected_store__allow_insecure_fallback: bool,
    protected_store__always_use_insecure_storage: bool,
    protected_store__insecure_fallback_directory: *mut wire_uint_8_list,
    protected_store__delete: bool,
    table_store__directory: *mut wire_uint_8_list,
    table_store__delete: bool,
    block_store__directory: *mut wire_uint_8_list,
    block_store__delete: bool,
    network__max_connections: u32,
    network__connection_initial_timeout_ms: u32,
    network__node_id: *mut wire_uint_8_list,
    network__node_id_secret: *mut wire_uint_8_list,
    network__bootstrap: *mut wire_StringList,
    network__upnp: bool,
    network__natpmp: bool,
    network__enable_local_peer_scope: bool,
    network__restricted_nat_retries: u32,
    network__rpc__concurrency: u32,
    network__rpc__queue_size: u32,
    network__rpc__max_timestamp_behind_ms: *mut u32,
    network__rpc__max_timestamp_ahead_ms: *mut u32,
    network__rpc__timeout_ms: u32,
    network__rpc__max_route_hop_count: u8,
    network__dht__resolve_node_timeout_ms: *mut u32,
    network__dht__resolve_node_count: u32,
    network__dht__resolve_node_fanout: u32,
    network__dht__max_find_node_count: u32,
    network__dht__get_value_timeout_ms: *mut u32,
    network__dht__get_value_count: u32,
    network__dht__get_value_fanout: u32,
    network__dht__set_value_timeout_ms: *mut u32,
    network__dht__set_value_count: u32,
    network__dht__set_value_fanout: u32,
    network__dht__min_peer_count: u32,
    network__dht__min_peer_refresh_time_ms: u32,
    network__dht__validate_dial_info_receipt_time_ms: u32,
    network__protocol__udp__enabled: bool,
    network__protocol__udp__socket_pool_size: u32,
    network__protocol__udp__listen_address: *mut wire_uint_8_list,
    network__protocol__udp__public_address: *mut wire_uint_8_list,
    network__protocol__tcp__connect: bool,
    network__protocol__tcp__listen: bool,
    network__protocol__tcp__max_connections: u32,
    network__protocol__tcp__listen_address: *mut wire_uint_8_list,
    network__protocol__tcp__public_address: *mut wire_uint_8_list,
    network__protocol__ws__connect: bool,
    network__protocol__ws__listen: bool,
    network__protocol__ws__max_connections: u32,
    network__protocol__ws__listen_address: *mut wire_uint_8_list,
    network__protocol__ws__path: *mut wire_uint_8_list,
    network__protocol__ws__url: *mut wire_uint_8_list,
    network__protocol__wss__connect: bool,
    network__protocol__wss__max_connections: u32,
    network__leases__max_server_signal_leases: u32,
    network__leases__max_server_relay_leases: u32,
    network__leases__max_client_signal_leases: u32,
    network__leases__max_client_relay_leases: u32,
}

// Section: wire enums

// Section: allocate functions

#[no_mangle]
pub extern "C" fn new_StringList(len: i32) -> *mut wire_StringList {
    let wrap = wire_StringList {
        ptr: support::new_leak_vec_ptr(<*mut wire_uint_8_list>::new_with_null_ptr(), len),
        len,
    };
    support::new_leak_box_ptr(wrap)
}

#[no_mangle]
pub extern "C" fn new_box_autoadd_u32(value: u32) -> *mut u32 {
    support::new_leak_box_ptr(value)
}

#[no_mangle]
pub extern "C" fn new_box_autoadd_veilid_config() -> *mut wire_VeilidConfig {
    support::new_leak_box_ptr(wire_VeilidConfig::new_with_null_ptr())
}

#[no_mangle]
pub extern "C" fn new_uint_8_list(len: i32) -> *mut wire_uint_8_list {
    let ans = wire_uint_8_list {
        ptr: support::new_leak_vec_ptr(Default::default(), len),
        len,
    };
    support::new_leak_box_ptr(ans)
}

// Section: impl Wire2Api

pub trait Wire2Api<T> {
    fn wire2api(self) -> T;
}

impl<T, S> Wire2Api<Option<T>> for *mut S
where
    *mut S: Wire2Api<T>,
{
    fn wire2api(self) -> Option<T> {
        if self.is_null() {
            None
        } else {
            Some(self.wire2api())
        }
    }
}

impl Wire2Api<String> for *mut wire_uint_8_list {
    fn wire2api(self) -> String {
        let vec: Vec<u8> = self.wire2api();
        String::from_utf8_lossy(&vec).into_owned()
    }
}

impl Wire2Api<Vec<String>> for *mut wire_StringList {
    fn wire2api(self) -> Vec<String> {
        let vec = unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        };
        vec.into_iter().map(Wire2Api::wire2api).collect()
    }
}

impl Wire2Api<bool> for bool {
    fn wire2api(self) -> bool {
        self
    }
}

impl Wire2Api<u32> for *mut u32 {
    fn wire2api(self) -> u32 {
        unsafe { *support::box_from_leak_ptr(self) }
    }
}

impl Wire2Api<VeilidConfig> for *mut wire_VeilidConfig {
    fn wire2api(self) -> VeilidConfig {
        let wrap = unsafe { support::box_from_leak_ptr(self) };
        (*wrap).wire2api().into()
    }
}

impl Wire2Api<u32> for u32 {
    fn wire2api(self) -> u32 {
        self
    }
}

impl Wire2Api<u8> for u8 {
    fn wire2api(self) -> u8 {
        self
    }
}

impl Wire2Api<Vec<u8>> for *mut wire_uint_8_list {
    fn wire2api(self) -> Vec<u8> {
        unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        }
    }
}

impl Wire2Api<VeilidConfig> for wire_VeilidConfig {
    fn wire2api(self) -> VeilidConfig {
        VeilidConfig {
            program_name: self.program_name.wire2api(),
            namespace: self.namespace.wire2api(),
            capabilities__protocol_udp: self.capabilities__protocol_udp.wire2api(),
            capabilities__protocol_connect_tcp: self.capabilities__protocol_connect_tcp.wire2api(),
            capabilities__protocol_accept_tcp: self.capabilities__protocol_accept_tcp.wire2api(),
            capabilities__protocol_connect_ws: self.capabilities__protocol_connect_ws.wire2api(),
            capabilities__protocol_accept_ws: self.capabilities__protocol_accept_ws.wire2api(),
            capabilities__protocol_connect_wss: self.capabilities__protocol_connect_wss.wire2api(),
            capabilities__protocol_accept_wss: self.capabilities__protocol_accept_wss.wire2api(),
            protected_store__allow_insecure_fallback: self
                .protected_store__allow_insecure_fallback
                .wire2api(),
            protected_store__always_use_insecure_storage: self
                .protected_store__always_use_insecure_storage
                .wire2api(),
            protected_store__insecure_fallback_directory: self
                .protected_store__insecure_fallback_directory
                .wire2api(),
            protected_store__delete: self.protected_store__delete.wire2api(),
            table_store__directory: self.table_store__directory.wire2api(),
            table_store__delete: self.table_store__delete.wire2api(),
            block_store__directory: self.block_store__directory.wire2api(),
            block_store__delete: self.block_store__delete.wire2api(),
            network__max_connections: self.network__max_connections.wire2api(),
            network__connection_initial_timeout_ms: self
                .network__connection_initial_timeout_ms
                .wire2api(),
            network__node_id: self.network__node_id.wire2api(),
            network__node_id_secret: self.network__node_id_secret.wire2api(),
            network__bootstrap: self.network__bootstrap.wire2api(),
            network__upnp: self.network__upnp.wire2api(),
            network__natpmp: self.network__natpmp.wire2api(),
            network__enable_local_peer_scope: self.network__enable_local_peer_scope.wire2api(),
            network__restricted_nat_retries: self.network__restricted_nat_retries.wire2api(),
            network__rpc__concurrency: self.network__rpc__concurrency.wire2api(),
            network__rpc__queue_size: self.network__rpc__queue_size.wire2api(),
            network__rpc__max_timestamp_behind_ms: self
                .network__rpc__max_timestamp_behind_ms
                .wire2api(),
            network__rpc__max_timestamp_ahead_ms: self
                .network__rpc__max_timestamp_ahead_ms
                .wire2api(),
            network__rpc__timeout_ms: self.network__rpc__timeout_ms.wire2api(),
            network__rpc__max_route_hop_count: self.network__rpc__max_route_hop_count.wire2api(),
            network__dht__resolve_node_timeout_ms: self
                .network__dht__resolve_node_timeout_ms
                .wire2api(),
            network__dht__resolve_node_count: self.network__dht__resolve_node_count.wire2api(),
            network__dht__resolve_node_fanout: self.network__dht__resolve_node_fanout.wire2api(),
            network__dht__max_find_node_count: self.network__dht__max_find_node_count.wire2api(),
            network__dht__get_value_timeout_ms: self.network__dht__get_value_timeout_ms.wire2api(),
            network__dht__get_value_count: self.network__dht__get_value_count.wire2api(),
            network__dht__get_value_fanout: self.network__dht__get_value_fanout.wire2api(),
            network__dht__set_value_timeout_ms: self.network__dht__set_value_timeout_ms.wire2api(),
            network__dht__set_value_count: self.network__dht__set_value_count.wire2api(),
            network__dht__set_value_fanout: self.network__dht__set_value_fanout.wire2api(),
            network__dht__min_peer_count: self.network__dht__min_peer_count.wire2api(),
            network__dht__min_peer_refresh_time_ms: self
                .network__dht__min_peer_refresh_time_ms
                .wire2api(),
            network__dht__validate_dial_info_receipt_time_ms: self
                .network__dht__validate_dial_info_receipt_time_ms
                .wire2api(),
            network__protocol__udp__enabled: self.network__protocol__udp__enabled.wire2api(),
            network__protocol__udp__socket_pool_size: self
                .network__protocol__udp__socket_pool_size
                .wire2api(),
            network__protocol__udp__listen_address: self
                .network__protocol__udp__listen_address
                .wire2api(),
            network__protocol__udp__public_address: self
                .network__protocol__udp__public_address
                .wire2api(),
            network__protocol__tcp__connect: self.network__protocol__tcp__connect.wire2api(),
            network__protocol__tcp__listen: self.network__protocol__tcp__listen.wire2api(),
            network__protocol__tcp__max_connections: self
                .network__protocol__tcp__max_connections
                .wire2api(),
            network__protocol__tcp__listen_address: self
                .network__protocol__tcp__listen_address
                .wire2api(),
            network__protocol__tcp__public_address: self
                .network__protocol__tcp__public_address
                .wire2api(),
            network__protocol__ws__connect: self.network__protocol__ws__connect.wire2api(),
            network__protocol__ws__listen: self.network__protocol__ws__listen.wire2api(),
            network__protocol__ws__max_connections: self
                .network__protocol__ws__max_connections
                .wire2api(),
            network__protocol__ws__listen_address: self
                .network__protocol__ws__listen_address
                .wire2api(),
            network__protocol__ws__path: self.network__protocol__ws__path.wire2api(),
            network__protocol__ws__url: self.network__protocol__ws__url.wire2api(),
            network__protocol__wss__connect: self.network__protocol__wss__connect.wire2api(),
            network__protocol__wss__max_connections: self
                .network__protocol__wss__max_connections
                .wire2api(),
            network__leases__max_server_signal_leases: self
                .network__leases__max_server_signal_leases
                .wire2api(),
            network__leases__max_server_relay_leases: self
                .network__leases__max_server_relay_leases
                .wire2api(),
            network__leases__max_client_signal_leases: self
                .network__leases__max_client_signal_leases
                .wire2api(),
            network__leases__max_client_relay_leases: self
                .network__leases__max_client_relay_leases
                .wire2api(),
        }
    }
}

// Section: impl NewWithNullPtr

pub trait NewWithNullPtr {
    fn new_with_null_ptr() -> Self;
}

impl<T> NewWithNullPtr for *mut T {
    fn new_with_null_ptr() -> Self {
        std::ptr::null_mut()
    }
}

impl NewWithNullPtr for wire_VeilidConfig {
    fn new_with_null_ptr() -> Self {
        Self {
            program_name: core::ptr::null_mut(),
            namespace: core::ptr::null_mut(),
            capabilities__protocol_udp: Default::default(),
            capabilities__protocol_connect_tcp: Default::default(),
            capabilities__protocol_accept_tcp: Default::default(),
            capabilities__protocol_connect_ws: Default::default(),
            capabilities__protocol_accept_ws: Default::default(),
            capabilities__protocol_connect_wss: Default::default(),
            capabilities__protocol_accept_wss: Default::default(),
            protected_store__allow_insecure_fallback: Default::default(),
            protected_store__always_use_insecure_storage: Default::default(),
            protected_store__insecure_fallback_directory: core::ptr::null_mut(),
            protected_store__delete: Default::default(),
            table_store__directory: core::ptr::null_mut(),
            table_store__delete: Default::default(),
            block_store__directory: core::ptr::null_mut(),
            block_store__delete: Default::default(),
            network__max_connections: Default::default(),
            network__connection_initial_timeout_ms: Default::default(),
            network__node_id: core::ptr::null_mut(),
            network__node_id_secret: core::ptr::null_mut(),
            network__bootstrap: core::ptr::null_mut(),
            network__upnp: Default::default(),
            network__natpmp: Default::default(),
            network__enable_local_peer_scope: Default::default(),
            network__restricted_nat_retries: Default::default(),
            network__rpc__concurrency: Default::default(),
            network__rpc__queue_size: Default::default(),
            network__rpc__max_timestamp_behind_ms: core::ptr::null_mut(),
            network__rpc__max_timestamp_ahead_ms: core::ptr::null_mut(),
            network__rpc__timeout_ms: Default::default(),
            network__rpc__max_route_hop_count: Default::default(),
            network__dht__resolve_node_timeout_ms: core::ptr::null_mut(),
            network__dht__resolve_node_count: Default::default(),
            network__dht__resolve_node_fanout: Default::default(),
            network__dht__max_find_node_count: Default::default(),
            network__dht__get_value_timeout_ms: core::ptr::null_mut(),
            network__dht__get_value_count: Default::default(),
            network__dht__get_value_fanout: Default::default(),
            network__dht__set_value_timeout_ms: core::ptr::null_mut(),
            network__dht__set_value_count: Default::default(),
            network__dht__set_value_fanout: Default::default(),
            network__dht__min_peer_count: Default::default(),
            network__dht__min_peer_refresh_time_ms: Default::default(),
            network__dht__validate_dial_info_receipt_time_ms: Default::default(),
            network__protocol__udp__enabled: Default::default(),
            network__protocol__udp__socket_pool_size: Default::default(),
            network__protocol__udp__listen_address: core::ptr::null_mut(),
            network__protocol__udp__public_address: core::ptr::null_mut(),
            network__protocol__tcp__connect: Default::default(),
            network__protocol__tcp__listen: Default::default(),
            network__protocol__tcp__max_connections: Default::default(),
            network__protocol__tcp__listen_address: core::ptr::null_mut(),
            network__protocol__tcp__public_address: core::ptr::null_mut(),
            network__protocol__ws__connect: Default::default(),
            network__protocol__ws__listen: Default::default(),
            network__protocol__ws__max_connections: Default::default(),
            network__protocol__ws__listen_address: core::ptr::null_mut(),
            network__protocol__ws__path: core::ptr::null_mut(),
            network__protocol__ws__url: core::ptr::null_mut(),
            network__protocol__wss__connect: Default::default(),
            network__protocol__wss__max_connections: Default::default(),
            network__leases__max_server_signal_leases: Default::default(),
            network__leases__max_server_relay_leases: Default::default(),
            network__leases__max_client_signal_leases: Default::default(),
            network__leases__max_client_relay_leases: Default::default(),
        }
    }
}

// Section: impl IntoDart

impl support::IntoDart for AttachmentState {
    fn into_dart(self) -> support::DartCObject {
        match self {
            Self::Detached => 0,
            Self::Attaching => 1,
            Self::AttachedWeak => 2,
            Self::AttachedGood => 3,
            Self::AttachedStrong => 4,
            Self::FullyAttached => 5,
            Self::OverAttached => 6,
            Self::Detaching => 7,
        }
        .into_dart()
    }
}

impl support::IntoDart for VeilidState {
    fn into_dart(self) -> support::DartCObject {
        vec![self.attachment.into_dart()].into_dart()
    }
}
impl support::IntoDartExceptPrimitive for VeilidState {}

impl support::IntoDart for VeilidUpdate {
    fn into_dart(self) -> support::DartCObject {
        match self {
            Self::Attachment(field0) => vec![0.into_dart(), field0.into_dart()],
        }
        .into_dart()
    }
}

impl support::IntoDart for VeilidVersion {
    fn into_dart(self) -> support::DartCObject {
        vec![
            self.major.into_dart(),
            self.minor.into_dart(),
            self.patch.into_dart(),
        ]
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for VeilidVersion {}

// Section: executor
support::lazy_static! {
    pub static ref FLUTTER_RUST_BRIDGE_HANDLER: support::DefaultHandler = Default::default();
}

// Section: sync execution mode utility

#[no_mangle]
pub extern "C" fn free_WireSyncReturnStruct(val: support::WireSyncReturnStruct) {
    unsafe {
        let _ = support::vec_from_leak_ptr(val.ptr, val.len);
    }
}
