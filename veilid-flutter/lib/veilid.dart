import 'dart:async';

import 'package:change_case/change_case.dart';

import 'veilid_stub.dart'
    if (dart.library.io) 'veilid_ffi.dart'
    if (dart.library.js) 'veilid_js.dart';

//////////////////////////////////////////////////////////

//////////////////////////////////////
/// JSON Encode Helper
Object? veilidApiToEncodable(Object? value) {
  if (value == null) {
    return value;
  }
  switch (value.runtimeType) {
    case AttachmentState:
      return (value as AttachmentState).json;
    case VeilidLogLevel:
      return (value as VeilidLogLevel).json;
    case VeilidConfigLogLevel:
      return (value as VeilidConfigLogLevel).json;
  }
  throw UnsupportedError('Cannot convert to JSON: $value');
}

//////////////////////////////////////
/// AttachmentState

enum AttachmentState {
  detached,
  attaching,
  attachedWeak,
  attachedGood,
  attachedStrong,
  fullyAttached,
  overAttached,
  detaching,
}

extension AttachmentStateExt on AttachmentState {
  String get json {
    return name.toPascalCase();
  }
}

AttachmentState attachmentStateFromJson(String j) {
  return AttachmentState.values.byName(j.toCamelCase());
}

//////////////////////////////////////
/// VeilidLogLevel

enum VeilidLogLevel {
  error,
  warn,
  info,
  debug,
  trace,
}

extension VeilidLogLevelExt on VeilidLogLevel {
  String get json {
    return name.toPascalCase();
  }
}

VeilidLogLevel veilidLogLevelFromJson(String j) {
  return VeilidLogLevel.values.byName(j.toCamelCase());
}

//////////////////////////////////////
/// VeilidConfigLogLevel

enum VeilidConfigLogLevel {
  off,
  error,
  warn,
  info,
  debug,
  trace,
}

extension VeilidConfigLogLevelExt on VeilidConfigLogLevel {
  String get json {
    return name.toPascalCase();
  }
}

VeilidConfigLogLevel veilidConfigLogLevelFromJson(String j) {
  return VeilidConfigLogLevel.values.byName(j.toCamelCase());
}

//////////////////////////////////////
/// VeilidConfig

class VeilidConfigHTTPS {
  bool enabled;
  String listenAddress;
  String path;
  String? url;

  VeilidConfigHTTPS({
    required this.enabled,
    required this.listenAddress,
    required this.path,
    this.url,
  });

  Map<String, dynamic> get json {
    return {
      'enabled': enabled,
      'listen_address': listenAddress,
      'path': path,
      'url': url
    };
  }

  VeilidConfigHTTPS.fromJson(Map<String, dynamic> json)
      : enabled = json['enabled'],
        listenAddress = json['listen_address'],
        path = json['path'],
        url = json['url'];
}

////////////

class VeilidConfigHTTP {
  bool enabled;
  String listenAddress;
  String path;
  String? url;

  VeilidConfigHTTP({
    required this.enabled,
    required this.listenAddress,
    required this.path,
    this.url,
  });

  Map<String, dynamic> get json {
    return {
      'enabled': enabled,
      'listen_address': listenAddress,
      'path': path,
      'url': url
    };
  }

  VeilidConfigHTTP.fromJson(Map<String, dynamic> json)
      : enabled = json['enabled'],
        listenAddress = json['listen_address'],
        path = json['path'],
        url = json['url'];
}

////////////

class VeilidConfigApplication {
  VeilidConfigHTTPS https;
  VeilidConfigHTTP http;

  VeilidConfigApplication({
    required this.https,
    required this.http,
  });

  Map<String, dynamic> get json {
    return {
      'https': https.json,
      'http': http.json,
    };
  }

  VeilidConfigApplication.fromJson(Map<String, dynamic> json)
      : https = VeilidConfigHTTPS.fromJson(json['https']),
        http = VeilidConfigHTTP.fromJson(json['http']);
}

////////////

class VeilidConfigUDP {
  bool enabled;
  int socketPoolSize;
  String listenAddress;
  String? publicAddress;

  VeilidConfigUDP(
      {required this.enabled,
      required this.socketPoolSize,
      required this.listenAddress,
      this.publicAddress});

  Map<String, dynamic> get json {
    return {
      'enabled': enabled,
      'socket_pool_size': socketPoolSize,
      'listen_address': listenAddress,
      'public_address': publicAddress,
    };
  }

  VeilidConfigUDP.fromJson(Map<String, dynamic> json)
      : enabled = json['enabled'],
        socketPoolSize = json['socket_pool_size'],
        listenAddress = json['listen_address'],
        publicAddress = json['publicAddress'];
}

////////////

class VeilidConfigTCP {
  bool connect;
  bool listen;
  int maxConnections;
  String listenAddress;
  String? publicAddress;

  VeilidConfigTCP(
      {required this.connect,
      required this.listen,
      required this.maxConnections,
      required this.listenAddress,
      this.publicAddress});

  Map<String, dynamic> get json {
    return {
      'connect': connect,
      'listen': listen,
      'max_connections': maxConnections,
      'listen_address': listenAddress,
      'public_address': publicAddress,
    };
  }

  VeilidConfigTCP.fromJson(Map<String, dynamic> json)
      : connect = json['connect'],
        listen = json['listen'],
        maxConnections = json['max_connections'],
        listenAddress = json['listen_address'],
        publicAddress = json['publicAddress'];
}

////////////

class VeilidConfigWS {
  bool connect;
  bool listen;
  int maxConnections;
  String listenAddress;
  String path;
  String? url;

  VeilidConfigWS(
      {required this.connect,
      required this.listen,
      required this.maxConnections,
      required this.listenAddress,
      required this.path,
      this.url});

  Map<String, dynamic> get json {
    return {
      'connect': connect,
      'listen': listen,
      'max_connections': maxConnections,
      'listen_address': listenAddress,
      'path': path,
      'url': url,
    };
  }

  VeilidConfigWS.fromJson(Map<String, dynamic> json)
      : connect = json['connect'],
        listen = json['listen'],
        maxConnections = json['max_connections'],
        listenAddress = json['listen_address'],
        path = json['path'],
        url = json['url'];
}

////////////

class VeilidConfigWSS {
  bool connect;
  bool listen;
  int maxConnections;
  String listenAddress;
  String path;
  String? url;

  VeilidConfigWSS(
      {required this.connect,
      required this.listen,
      required this.maxConnections,
      required this.listenAddress,
      required this.path,
      this.url});

  Map<String, dynamic> get json {
    return {
      'connect': connect,
      'listen': listen,
      'max_connections': maxConnections,
      'listen_address': listenAddress,
      'path': path,
      'url': url,
    };
  }

  VeilidConfigWSS.fromJson(Map<String, dynamic> json)
      : connect = json['connect'],
        listen = json['listen'],
        maxConnections = json['max_connections'],
        listenAddress = json['listen_address'],
        path = json['path'],
        url = json['url'];
}

////////////

class VeilidConfigProtocol {
  VeilidConfigUDP udp;
  VeilidConfigTCP tcp;
  VeilidConfigWS ws;
  VeilidConfigWSS wss;

  VeilidConfigProtocol({
    required this.udp,
    required this.tcp,
    required this.ws,
    required this.wss,
  });

  Map<String, dynamic> get json {
    return {
      'udp': udp.json,
      'tcp': tcp.json,
      'ws': ws.json,
      'wss': wss.json,
    };
  }

  VeilidConfigProtocol.fromJson(Map<String, dynamic> json)
      : udp = VeilidConfigUDP.fromJson(json['udp']),
        tcp = VeilidConfigTCP.fromJson(json['tcp']),
        ws = VeilidConfigWS.fromJson(json['ws']),
        wss = VeilidConfigWSS.fromJson(json['wss']);
}

////////////

class VeilidConfigTLS {
  String certificatePath;
  String privateKeyPath;
  int connectionInitialTimeoutMs;

  VeilidConfigTLS({
    required this.certificatePath,
    required this.privateKeyPath,
    required this.connectionInitialTimeoutMs,
  });

  Map<String, dynamic> get json {
    return {
      'certificate_path': certificatePath,
      'private_key_path': privateKeyPath,
      'connection_initial_timeout_ms': connectionInitialTimeoutMs,
    };
  }

  VeilidConfigTLS.fromJson(Map<String, dynamic> json)
      : certificatePath = json['certificate_path'],
        privateKeyPath = json['private_key_path'],
        connectionInitialTimeoutMs = json['connection_initial_timeout_ms'];
}

////////////

class VeilidConfigDHT {
  int? resolveNodeTimeoutMs;
  int resolveNodeCount;
  int resolveNodeFanout;
  int maxFindNodeCount;
  int? getValueTimeoutMs;
  int getValueCount;
  int getValueFanout;
  int? setValueTimeoutMs;
  int setValueCount;
  int setValueFanout;
  int minPeerCount;
  int minPeerRefreshTimeMs;
  int validateDialInfoReceiptTimeMs;

  VeilidConfigDHT(
      {this.resolveNodeTimeoutMs,
      required this.resolveNodeCount,
      required this.resolveNodeFanout,
      required this.maxFindNodeCount,
      this.getValueTimeoutMs,
      required this.getValueCount,
      required this.getValueFanout,
      this.setValueTimeoutMs,
      required this.setValueCount,
      required this.setValueFanout,
      required this.minPeerCount,
      required this.minPeerRefreshTimeMs,
      required this.validateDialInfoReceiptTimeMs});

  Map<String, dynamic> get json {
    return {
      'resolve_node_timeout_ms': resolveNodeTimeoutMs,
      'resolve_node_count': resolveNodeCount,
      'resolve_node_fanout': resolveNodeFanout,
      'max_find_node_count': maxFindNodeCount,
      'get_value_timeout_ms': getValueTimeoutMs,
      'get_value_count': getValueCount,
      'get_value_fanout': getValueFanout,
      'set_value_timeout_ms': setValueTimeoutMs,
      'set_value_count': setValueCount,
      'set_value_fanout': setValueFanout,
      'min_peer_count': minPeerCount,
      'min_peer_refresh_time_ms': minPeerRefreshTimeMs,
      'validate_dial_info_receipt_time_ms': validateDialInfoReceiptTimeMs
    };
  }

  VeilidConfigDHT.fromJson(Map<String, dynamic> json)
      : resolveNodeTimeoutMs = json['resolve_node_timeout_ms'],
        resolveNodeCount = json['resolve_node_count'],
        resolveNodeFanout = json['resolve_node_fanout'],
        maxFindNodeCount = json['max_find_node_count'],
        getValueTimeoutMs = json['get_value_timeout_ms'],
        getValueCount = json['get_value_count'],
        getValueFanout = json['get_value_fanout'],
        setValueTimeoutMs = json['set_value_timeout_ms'],
        setValueCount = json['set_value_count'],
        setValueFanout = json['set_value_fanout'],
        minPeerCount = json['min_peer_count'],
        minPeerRefreshTimeMs = json['min_peer_refresh_time_ms'],
        validateDialInfoReceiptTimeMs =
            json['validate_dial_info_receipt_time_ms'];
}

////////////

class VeilidConfigRPC {
  int concurrency;
  int queueSize;
  int? maxTimestampBehindMs;
  int? maxTimestampAheadMs;
  int timeoutMs;
  int maxRouteHopCount;

  VeilidConfigRPC(
      {required this.concurrency,
      required this.queueSize,
      this.maxTimestampBehindMs,
      this.maxTimestampAheadMs,
      required this.timeoutMs,
      required this.maxRouteHopCount});

  Map<String, dynamic> get json {
    return {
      'concurrency': concurrency,
      'queue_size': queueSize,
      'max_timestamp_behind_ms': maxTimestampBehindMs,
      'max_timestamp_ahead_ms': maxTimestampAheadMs,
      'timeout_ms': timeoutMs,
      'max_route_hop_count': maxRouteHopCount,
    };
  }

  VeilidConfigRPC.fromJson(Map<String, dynamic> json)
      : concurrency = json['concurrency'],
        queueSize = json['queue_size'],
        maxTimestampBehindMs = json['max_timestamp_behind_ms'],
        maxTimestampAheadMs = json['max_timestamp_ahead_ms'],
        timeoutMs = json['timeout_ms'],
        maxRouteHopCount = json['max_route_hop_count'];
}

////////////

class VeilidConfigRoutingTable {
  int limitOverAttached;
  int limitFullyAttached;
  int limitAttachedStrong;
  int limitAttachedGood;
  int limitAttachedWeak;

  VeilidConfigRoutingTable({
    required this.limitOverAttached,
    required this.limitFullyAttached,
    required this.limitAttachedStrong,
    required this.limitAttachedGood,
    required this.limitAttachedWeak,
  });

  Map<String, dynamic> get json {
    return {
      'limit_over_attached': limitOverAttached,
      'limit_fully_attached': limitFullyAttached,
      'limit_attached_strong': limitAttachedStrong,
      'limit_attached_good': limitAttachedGood,
      'limit_attached_weak': limitAttachedWeak,
    };
  }

  VeilidConfigRoutingTable.fromJson(Map<String, dynamic> json)
      : limitOverAttached = json['limit_over_attached'],
        limitFullyAttached = json['limit_fully_attached'],
        limitAttachedStrong = json['limit_attached_strong'],
        limitAttachedGood = json['limit_attached_good'],
        limitAttachedWeak = json['limit_attached_weak'];
}

////////////

class VeilidConfigNetwork {
  int connectionInitialTimeoutMs;
  int connectionInactivityTimeoutMs;
  int maxConnectionsPerIp4;
  int maxConnectionsPerIp6Prefix;
  int maxConnectionsPerIp6PrefixSize;
  int maxConnectionFrequencyPerMin;
  int clientWhitelistTimeoutMs;
  int reverseConnectionReceiptTimeMs;
  int holePunchReceiptTimeMs;
  String nodeId;
  String nodeIdSecret;
  List<String> bootstrap;
  List<String> bootstrapNodes;
  VeilidConfigRoutingTable routingTable;
  VeilidConfigRPC rpc;
  VeilidConfigDHT dht;
  bool upnp;
  bool natpmp;
  bool enableLocalPeerScope;
  int restrictedNatRetries;
  VeilidConfigTLS tls;
  VeilidConfigApplication application;
  VeilidConfigProtocol protocol;

  VeilidConfigNetwork({
    required this.connectionInitialTimeoutMs,
    required this.connectionInactivityTimeoutMs,
    required this.maxConnectionsPerIp4,
    required this.maxConnectionsPerIp6Prefix,
    required this.maxConnectionsPerIp6PrefixSize,
    required this.maxConnectionFrequencyPerMin,
    required this.clientWhitelistTimeoutMs,
    required this.reverseConnectionReceiptTimeMs,
    required this.holePunchReceiptTimeMs,
    required this.nodeId,
    required this.nodeIdSecret,
    required this.bootstrap,
    required this.bootstrapNodes,
    required this.routingTable,
    required this.rpc,
    required this.dht,
    required this.upnp,
    required this.natpmp,
    required this.enableLocalPeerScope,
    required this.restrictedNatRetries,
    required this.tls,
    required this.application,
    required this.protocol,
  });

  Map<String, dynamic> get json {
    return {
      'connection_initial_timeout_ms': connectionInitialTimeoutMs,
      'connection_inactivity_timeout_ms': connectionInactivityTimeoutMs,
      'max_connections_per_ip4': maxConnectionsPerIp4,
      'max_connections_per_ip6_prefix': maxConnectionsPerIp6Prefix,
      'max_connections_per_ip6_prefix_size': maxConnectionsPerIp6PrefixSize,
      'max_connection_frequency_per_min': maxConnectionFrequencyPerMin,
      'client_whitelist_timeout_ms': clientWhitelistTimeoutMs,
      'reverse_connection_receipt_time_ms': reverseConnectionReceiptTimeMs,
      'hole_punch_receipt_time_ms': holePunchReceiptTimeMs,
      'node_id': nodeId,
      'node_id_secret': nodeIdSecret,
      'bootstrap': bootstrap,
      'bootstrap_nodes': bootstrapNodes,
      'routing_table': routingTable.json,
      'rpc': rpc.json,
      'dht': dht.json,
      'upnp': upnp,
      'natpmp': natpmp,
      'enable_local_peer_scope': enableLocalPeerScope,
      'restricted_nat_retries': restrictedNatRetries,
      'tls': tls.json,
      'application': application.json,
      'protocol': protocol.json,
    };
  }

  VeilidConfigNetwork.fromJson(Map<String, dynamic> json)
      : connectionInitialTimeoutMs = json['connection_initial_timeout_ms'],
        connectionInactivityTimeoutMs =
            json['connection_inactivity_timeout_ms'],
        maxConnectionsPerIp4 = json['max_connections_per_ip4'],
        maxConnectionsPerIp6Prefix = json['max_connections_per_ip6_prefix'],
        maxConnectionsPerIp6PrefixSize =
            json['max_connections_per_ip6_prefix_size'],
        maxConnectionFrequencyPerMin = json['max_connection_frequency_per_min'],
        clientWhitelistTimeoutMs = json['client_whitelist_timeout_ms'],
        reverseConnectionReceiptTimeMs =
            json['reverse_connection_receipt_time_ms'],
        holePunchReceiptTimeMs = json['hole_punch_receipt_time_ms'],
        nodeId = json['node_id'],
        nodeIdSecret = json['node_id_secret'],
        bootstrap = json['bootstrap'],
        bootstrapNodes = json['bootstrap_nodes'],
        routingTable = VeilidConfigRoutingTable.fromJson(json['routing_table']),
        rpc = VeilidConfigRPC.fromJson(json['rpc']),
        dht = VeilidConfigDHT.fromJson(json['dht']),
        upnp = json['upnp'],
        natpmp = json['natpmp'],
        enableLocalPeerScope = json['enable_local_peer_scope'],
        restrictedNatRetries = json['restricted_nat_retries'],
        tls = VeilidConfigTLS.fromJson(json['tls']),
        application = VeilidConfigApplication.fromJson(json['application']),
        protocol = VeilidConfigProtocol.fromJson(json['protocol']);
}

////////////

class VeilidConfigTableStore {
  String directory;
  bool delete;

  VeilidConfigTableStore({
    required this.directory,
    required this.delete,
  });

  Map<String, dynamic> get json {
    return {'directory': directory, 'delete': delete};
  }

  VeilidConfigTableStore.fromJson(Map<String, dynamic> json)
      : directory = json['directory'],
        delete = json['delete'];
}

////////////

class VeilidConfigBlockStore {
  String directory;
  bool delete;

  VeilidConfigBlockStore({
    required this.directory,
    required this.delete,
  });

  Map<String, dynamic> get json {
    return {'directory': directory, 'delete': delete};
  }

  VeilidConfigBlockStore.fromJson(Map<String, dynamic> json)
      : directory = json['directory'],
        delete = json['delete'];
}

////////////

class VeilidConfigProtectedStore {
  bool allowInsecureFallback;
  bool alwaysUseInsecureStorage;
  String insecureFallbackDirectory;
  bool delete;

  VeilidConfigProtectedStore({
    required this.allowInsecureFallback,
    required this.alwaysUseInsecureStorage,
    required this.insecureFallbackDirectory,
    required this.delete,
  });

  Map<String, dynamic> get json {
    return {
      'allow_insecure_fallback': allowInsecureFallback,
      'always_use_insecure_storage': alwaysUseInsecureStorage,
      'insecure_fallback_directory': insecureFallbackDirectory,
      'delete': delete,
    };
  }

  VeilidConfigProtectedStore.fromJson(Map<String, dynamic> json)
      : allowInsecureFallback = json['allow_insecure_fallback'],
        alwaysUseInsecureStorage = json['always_use_insecure_storage'],
        insecureFallbackDirectory = json['insecure_fallback_directory'],
        delete = json['delete'];
}

////////////

class VeilidConfigCapabilities {
  bool protocolUDP;
  bool protocolConnectTCP;
  bool protocolAcceptTCP;
  bool protocolConnectWS;
  bool protocolAcceptWS;
  bool protocolConnectWSS;
  bool protocolAcceptWSS;

  VeilidConfigCapabilities({
    required this.protocolUDP,
    required this.protocolConnectTCP,
    required this.protocolAcceptTCP,
    required this.protocolConnectWS,
    required this.protocolAcceptWS,
    required this.protocolConnectWSS,
    required this.protocolAcceptWSS,
  });

  Map<String, dynamic> get json {
    return {
      'protocol_udp': protocolUDP,
      'protocol_connect_tcp': protocolConnectTCP,
      'protocol_accept_tcp': protocolAcceptTCP,
      'protocol_connect_ws': protocolConnectWS,
      'protocol_accept_ws': protocolAcceptWS,
      'protocol_connect_wss': protocolConnectWSS,
      'protocol_accept_wss': protocolAcceptWSS,
    };
  }

  VeilidConfigCapabilities.fromJson(Map<String, dynamic> json)
      : protocolUDP = json['protocol_udp'],
        protocolConnectTCP = json['protocol_connect_tcp'],
        protocolAcceptTCP = json['protocol_accept_tcp'],
        protocolConnectWS = json['protocol_connect_ws'],
        protocolAcceptWS = json['protocol_accept_ws'],
        protocolConnectWSS = json['protocol_connect_wss'],
        protocolAcceptWSS = json['protocol_accept_wss'];
}

////////////

class VeilidConfig {
  String programName;
  String namespace;
  VeilidConfigLogLevel apiLogLevel;
  VeilidConfigCapabilities capabilities;
  VeilidConfigProtectedStore protectedStore;
  VeilidConfigTableStore tableStore;
  VeilidConfigBlockStore blockStore;
  VeilidConfigNetwork network;

  VeilidConfig({
    required this.programName,
    required this.namespace,
    required this.apiLogLevel,
    required this.capabilities,
    required this.protectedStore,
    required this.tableStore,
    required this.blockStore,
    required this.network,
  });

  Map<String, dynamic> get json {
    return {
      'program_name': programName,
      'namespace': namespace,
      'api_log_level': apiLogLevel.json,
      'capabilities': capabilities.json,
      'protected_store': protectedStore.json,
      'table_store': tableStore.json,
      'block_store': blockStore.json,
      'network': network.json
    };
  }

  VeilidConfig.fromJson(Map<String, dynamic> json)
      : programName = json['program_name'],
        namespace = json['namespace'],
        apiLogLevel = json['api_log_level'],
        capabilities = VeilidConfigCapabilities.fromJson(json['capabilities']),
        protectedStore =
            VeilidConfigProtectedStore.fromJson(json['protected_store']),
        tableStore = VeilidConfigTableStore.fromJson(json['table_store']),
        blockStore = VeilidConfigBlockStore.fromJson(json['block_store']),
        network = VeilidConfigNetwork.fromJson(json['network']);
}

//////////////////////////////////////
/// VeilidUpdate

abstract class VeilidUpdate {
  factory VeilidUpdate.fromJson(Map<String, dynamic> json) {
    switch (json["kind"]) {
      case "Log":
        {
          return VeilidUpdateLog(
              veilidLogLevelFromJson(json["api_log_level"]), json["message"]);
        }
      case "Attachment":
        {
          return VeilidUpdateAttachment(attachmentStateFromJson(json["state"]));
        }
      case "Network":
        {
          return VeilidUpdateNetwork(
              json["started"], json["bps_up"], json["bps_down"]);
        }
      default:
        {
          throw VeilidAPIExceptionInternal(
              "Invalid VeilidAPIException type: ${json['kind']}");
        }
    }
  }
  Map<String, dynamic> get json;
}

class VeilidUpdateLog implements VeilidUpdate {
  final VeilidLogLevel logLevel;
  final String message;
  //
  VeilidUpdateLog(this.logLevel, this.message);

  @override
  Map<String, dynamic> get json {
    return {
      'kind': "Log",
      'log_level': logLevel.json,
      'message': message,
    };
  }
}

class VeilidUpdateAttachment implements VeilidUpdate {
  final AttachmentState state;
  //
  VeilidUpdateAttachment(this.state);

  @override
  Map<String, dynamic> get json {
    return {
      'kind': "Attachment",
      'state': state.json,
    };
  }
}

class VeilidUpdateNetwork implements VeilidUpdate {
  final bool started;
  final int bpsDown;
  final int bpsUp;
  //
  VeilidUpdateNetwork(this.started, this.bpsDown, this.bpsUp);

  @override
  Map<String, dynamic> get json {
    return {
      'kind': "Network",
      'started': started,
      'bps_down': bpsDown,
      'bps_up': bpsUp
    };
  }
}

//////////////////////////////////////
/// VeilidStateAttachment

class VeilidStateAttachment {
  final AttachmentState state;

  VeilidStateAttachment(this.state);

  VeilidStateAttachment.fromJson(Map<String, dynamic> json)
      : state = attachmentStateFromJson(json['state']);
}

//////////////////////////////////////
/// VeilidStateNetwork

class VeilidStateNetwork {
  final bool started;

  VeilidStateNetwork(this.started);

  VeilidStateNetwork.fromJson(Map<String, dynamic> json)
      : started = json['started'];
}

//////////////////////////////////////
/// VeilidState

class VeilidState {
  final VeilidStateAttachment attachment;
  final VeilidStateNetwork network;

  VeilidState(this.attachment, this.network);

  VeilidState.fromJson(Map<String, dynamic> json)
      : attachment = VeilidStateAttachment.fromJson(json['attachment']),
        network = VeilidStateNetwork.fromJson(json['network']);
}

//////////////////////////////////////
/// VeilidAPIException

abstract class VeilidAPIException implements Exception {
  factory VeilidAPIException.fromJson(Map<String, dynamic> json) {
    switch (json["kind"]) {
      case "NotInitialized":
        {
          return VeilidAPIExceptionNotInitialized();
        }
      case "AlreadyInitialized":
        {
          return VeilidAPIExceptionAlreadyInitialized();
        }
      case "Timeout":
        {
          return VeilidAPIExceptionTimeout();
        }
      case "Shutdown":
        {
          return VeilidAPIExceptionShutdown();
        }
      case "NodeNotFound":
        {
          return VeilidAPIExceptionNodeNotFound(json["node_id"]);
        }
      case "NoDialInfo":
        {
          return VeilidAPIExceptionNoDialInfo(json["node_id"]);
        }
      case "Internal":
        {
          return VeilidAPIExceptionInternal(json["message"]);
        }
      case "Unimplemented":
        {
          return VeilidAPIExceptionUnimplemented(json["unimplemented"]);
        }
      case "ParseError":
        {
          return VeilidAPIExceptionParseError(json["message"], json["value"]);
        }
      case "InvalidArgument":
        {
          return VeilidAPIExceptionInvalidArgument(
              json["context"], json["argument"], json["value"]);
        }
      case "MissingArgument":
        {
          return VeilidAPIExceptionMissingArgument(
              json["context"], json["argument"]);
        }
      default:
        {
          throw VeilidAPIExceptionInternal(
              "Invalid VeilidAPIException type: ${json['kind']}");
        }
    }
  }
}

class VeilidAPIExceptionNotInitialized implements VeilidAPIException {
  @override
  String toString() {
    return "VeilidAPIException: NotInitialized";
  }
}

class VeilidAPIExceptionAlreadyInitialized implements VeilidAPIException {
  @override
  String toString() {
    return "VeilidAPIException: AlreadyInitialized";
  }
}

class VeilidAPIExceptionTimeout implements VeilidAPIException {
  @override
  String toString() {
    return "VeilidAPIException: Timeout";
  }
}

class VeilidAPIExceptionShutdown implements VeilidAPIException {
  @override
  String toString() {
    return "VeilidAPIException: Shutdown";
  }
}

class VeilidAPIExceptionNodeNotFound implements VeilidAPIException {
  final String nodeId;

  @override
  String toString() {
    return "VeilidAPIException: NodeNotFound (nodeId: $nodeId)";
  }

  //
  VeilidAPIExceptionNodeNotFound(this.nodeId);
}

class VeilidAPIExceptionNoDialInfo implements VeilidAPIException {
  final String nodeId;

  @override
  String toString() {
    return "VeilidAPIException: NoDialInfo (nodeId: $nodeId)";
  }

  //
  VeilidAPIExceptionNoDialInfo(this.nodeId);
}

class VeilidAPIExceptionInternal implements VeilidAPIException {
  final String message;

  @override
  String toString() {
    return "VeilidAPIException: Internal ($message)";
  }

  //
  VeilidAPIExceptionInternal(this.message);
}

class VeilidAPIExceptionUnimplemented implements VeilidAPIException {
  final String message;

  @override
  String toString() {
    return "VeilidAPIException: Unimplemented ($message)";
  }

  //
  VeilidAPIExceptionUnimplemented(this.message);
}

class VeilidAPIExceptionParseError implements VeilidAPIException {
  final String message;
  final String value;

  @override
  String toString() {
    return "VeilidAPIException: ParseError ($message)\n    value: $value";
  }

  //
  VeilidAPIExceptionParseError(this.message, this.value);
}

class VeilidAPIExceptionInvalidArgument implements VeilidAPIException {
  final String context;
  final String argument;
  final String value;

  @override
  String toString() {
    return "VeilidAPIException: InvalidArgument ($context:$argument)\n    value: $value";
  }

  //
  VeilidAPIExceptionInvalidArgument(this.context, this.argument, this.value);
}

class VeilidAPIExceptionMissingArgument implements VeilidAPIException {
  final String context;
  final String argument;

  @override
  String toString() {
    return "VeilidAPIException: MissingArgument ($context:$argument)";
  }

  //
  VeilidAPIExceptionMissingArgument(this.context, this.argument);
}

//////////////////////////////////////
/// VeilidVersion

class VeilidVersion {
  final int major;
  final int minor;
  final int patch;

  VeilidVersion(this.major, this.minor, this.patch);
}

//////////////////////////////////////
/// Veilid singleton factory

abstract class Veilid {
  static late Veilid instance = getVeilid();

  Stream<VeilidUpdate> startupVeilidCore(VeilidConfig config);
  Future<VeilidState> getVeilidState();
  Future<void> changeApiLogLevel(VeilidConfigLogLevel logLevel);
  Future<void> shutdownVeilidCore();
  Future<String> debug(String command);
  String veilidVersionString();
  VeilidVersion veilidVersion();
}
