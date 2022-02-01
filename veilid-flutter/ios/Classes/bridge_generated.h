#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct wire_uint_8_list {
  uint8_t *ptr;
  int32_t len;
} wire_uint_8_list;

typedef struct wire_StringList {
  struct wire_uint_8_list **ptr;
  int32_t len;
} wire_StringList;

typedef struct wire_VeilidConfig {
  struct wire_uint_8_list *program_name;
  struct wire_uint_8_list *veilid_namespace;
  int32_t api_log_level;
  bool capabilities__protocol_udp;
  bool capabilities__protocol_connect_tcp;
  bool capabilities__protocol_accept_tcp;
  bool capabilities__protocol_connect_ws;
  bool capabilities__protocol_accept_ws;
  bool capabilities__protocol_connect_wss;
  bool capabilities__protocol_accept_wss;
  bool protected_store__allow_insecure_fallback;
  bool protected_store__always_use_insecure_storage;
  struct wire_uint_8_list *protected_store__insecure_fallback_directory;
  bool protected_store__delete;
  struct wire_uint_8_list *table_store__directory;
  bool table_store__delete;
  struct wire_uint_8_list *block_store__directory;
  bool block_store__delete;
  uint32_t network__max_connections;
  uint32_t network__connection_initial_timeout_ms;
  struct wire_uint_8_list *network__node_id;
  struct wire_uint_8_list *network__node_id_secret;
  struct wire_StringList *network__bootstrap;
  bool network__upnp;
  bool network__natpmp;
  bool network__enable_local_peer_scope;
  uint32_t network__restricted_nat_retries;
  uint32_t network__rpc__concurrency;
  uint32_t network__rpc__queue_size;
  uint32_t *network__rpc__max_timestamp_behind_ms;
  uint32_t *network__rpc__max_timestamp_ahead_ms;
  uint32_t network__rpc__timeout_ms;
  uint8_t network__rpc__max_route_hop_count;
  uint32_t *network__dht__resolve_node_timeout_ms;
  uint32_t network__dht__resolve_node_count;
  uint32_t network__dht__resolve_node_fanout;
  uint32_t network__dht__max_find_node_count;
  uint32_t *network__dht__get_value_timeout_ms;
  uint32_t network__dht__get_value_count;
  uint32_t network__dht__get_value_fanout;
  uint32_t *network__dht__set_value_timeout_ms;
  uint32_t network__dht__set_value_count;
  uint32_t network__dht__set_value_fanout;
  uint32_t network__dht__min_peer_count;
  uint32_t network__dht__min_peer_refresh_time_ms;
  uint32_t network__dht__validate_dial_info_receipt_time_ms;
  bool network__protocol__udp__enabled;
  uint32_t network__protocol__udp__socket_pool_size;
  struct wire_uint_8_list *network__protocol__udp__listen_address;
  struct wire_uint_8_list *network__protocol__udp__public_address;
  bool network__protocol__tcp__connect;
  bool network__protocol__tcp__listen;
  uint32_t network__protocol__tcp__max_connections;
  struct wire_uint_8_list *network__protocol__tcp__listen_address;
  struct wire_uint_8_list *network__protocol__tcp__public_address;
  bool network__protocol__ws__connect;
  bool network__protocol__ws__listen;
  uint32_t network__protocol__ws__max_connections;
  struct wire_uint_8_list *network__protocol__ws__listen_address;
  struct wire_uint_8_list *network__protocol__ws__path;
  struct wire_uint_8_list *network__protocol__ws__url;
  bool network__protocol__wss__connect;
  uint32_t network__protocol__wss__max_connections;
  uint32_t network__leases__max_server_signal_leases;
  uint32_t network__leases__max_server_relay_leases;
  uint32_t network__leases__max_client_signal_leases;
  uint32_t network__leases__max_client_relay_leases;
} wire_VeilidConfig;

typedef struct WireSyncReturnStruct {
  uint8_t *ptr;
  int32_t len;
  bool success;
} WireSyncReturnStruct;

typedef int64_t DartPort;

typedef bool (*DartPostCObjectFnType)(DartPort port_id, void *message);

void wire_startup_veilid_core(int64_t port_, struct wire_VeilidConfig *config);

void wire_get_veilid_state(int64_t port_);

void wire_change_api_log_level(int64_t port_, int32_t log_level);

void wire_shutdown_veilid_core(int64_t port_);

void wire_veilid_version_string(int64_t port_);

void wire_veilid_version(int64_t port_);

struct wire_StringList *new_StringList(int32_t len);

uint32_t *new_box_autoadd_u32(uint32_t value);

struct wire_VeilidConfig *new_box_autoadd_veilid_config(void);

struct wire_uint_8_list *new_uint_8_list(int32_t len);

void free_WireSyncReturnStruct(struct WireSyncReturnStruct val);

void store_dart_post_cobject(DartPostCObjectFnType ptr);

static int64_t dummy_method_to_enforce_bundling(void) {
    int64_t dummy_var = 0;
    dummy_var ^= ((int64_t) (void*) wire_startup_veilid_core);
    dummy_var ^= ((int64_t) (void*) wire_get_veilid_state);
    dummy_var ^= ((int64_t) (void*) wire_change_api_log_level);
    dummy_var ^= ((int64_t) (void*) wire_shutdown_veilid_core);
    dummy_var ^= ((int64_t) (void*) wire_veilid_version_string);
    dummy_var ^= ((int64_t) (void*) wire_veilid_version);
    dummy_var ^= ((int64_t) (void*) new_StringList);
    dummy_var ^= ((int64_t) (void*) new_box_autoadd_u32);
    dummy_var ^= ((int64_t) (void*) new_box_autoadd_veilid_config);
    dummy_var ^= ((int64_t) (void*) new_uint_8_list);
    dummy_var ^= ((int64_t) (void*) free_WireSyncReturnStruct);
    dummy_var ^= ((int64_t) (void*) store_dart_post_cobject);
    return dummy_var;
}