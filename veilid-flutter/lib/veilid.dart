import 'dart:async';
import 'dart:convert';

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
/// VeilidConfig

class VeilidConfig {
  String programName;
  String veilidNamespace;
  VeilidLogLevel apiLogLevel;
  bool capabilitiesProtocolUdp;
  bool capabilitiesProtocolConnectTcp;
  bool capabilitiesProtocolAcceptTcp;
  bool capabilitiesProtocolConnectWs;
  bool capabilitiesProtocolAcceptWs;
  bool capabilitiesProtocolConnectWss;
  bool capabilitiesProtocolAcceptWss;
  bool protectedStoreAllowInsecureFallback;
  bool protectedStoreAlwaysUseInsecureStorage;
  String protectedStoreInsecureFallbackDirectory;
  bool protectedStoreDelete;
  String tableStoreDirectory;
  bool tableStoreDelete;
  String blockStoreDirectory;
  bool blockStoreDelete;
  int networkMaxConnections;
  int networkConnectionInitialTimeoutMs;
  String networkNodeId;
  String networkNodeIdSecret;
  List<String> networkBootstrap;
  bool networkUpnp;
  bool networkNatpmp;
  bool networkEnableLocalPeerScope;
  int networkRestrictedNatRetries;
  int networkRpcConcurrency;
  int networkRpcQueueSize;
  int? networkRpcMaxTimestampBehindMs;
  int? networkRpcMaxTimestampAheadMs;
  int networkRpcTimeoutMs;
  int networkRpcMaxRouteHopCount;
  int? networkDhtResolveNodeTimeoutMs;
  int networkDhtResolveNodeCount;
  int networkDhtResolveNodeFanout;
  int networkDhtMaxFindNodeCount;
  int? networkDhtGetValueTimeoutMs;
  int networkDhtGetValueCount;
  int networkDhtGetValueFanout;
  int? networkDhtSetValueTimeoutMs;
  int networkDhtSetValueCount;
  int networkDhtSetValueFanout;
  int networkDhtMinPeerCount;
  int networkDhtMinPeerRefreshTimeMs;
  int networkDhtValidateDialInfoReceiptTimeMs;
  bool networkProtocolUdpEnabled;
  int networkProtocolUdpSocketPoolSize;
  String networkProtocolUdpListenAddress;
  String? networkProtocolUdpPublicAddress;
  bool networkProtocolTcpConnect;
  bool networkProtocolTcpListen;
  int networkProtocolTcpMaxConnections;
  String networkProtocolTcpListenAddress;
  String? networkProtocolTcpPublicAddress;
  bool networkProtocolWsConnect;
  bool networkProtocolWsListen;
  int networkProtocolWsMaxConnections;
  String networkProtocolWsListenAddress;
  String networkProtocolWsPath;
  String? networkProtocolWsUrl;
  bool networkProtocolWssConnect;
  int networkProtocolWssMaxConnections;
  int networkLeasesMaxServerSignalLeases;
  int networkLeasesMaxServerRelayLeases;
  int networkLeasesMaxClientSignalLeases;
  int networkLeasesMaxClientRelayLeases;

  VeilidConfig({
    required this.programName,
    required this.veilidNamespace,
    required this.apiLogLevel,
    required this.capabilitiesProtocolUdp,
    required this.capabilitiesProtocolConnectTcp,
    required this.capabilitiesProtocolAcceptTcp,
    required this.capabilitiesProtocolConnectWs,
    required this.capabilitiesProtocolAcceptWs,
    required this.capabilitiesProtocolConnectWss,
    required this.capabilitiesProtocolAcceptWss,
    required this.protectedStoreAllowInsecureFallback,
    required this.protectedStoreAlwaysUseInsecureStorage,
    required this.protectedStoreInsecureFallbackDirectory,
    required this.protectedStoreDelete,
    required this.tableStoreDirectory,
    required this.tableStoreDelete,
    required this.blockStoreDirectory,
    required this.blockStoreDelete,
    required this.networkMaxConnections,
    required this.networkConnectionInitialTimeoutMs,
    required this.networkNodeId,
    required this.networkNodeIdSecret,
    required this.networkBootstrap,
    required this.networkUpnp,
    required this.networkNatpmp,
    required this.networkEnableLocalPeerScope,
    required this.networkRestrictedNatRetries,
    required this.networkRpcConcurrency,
    required this.networkRpcQueueSize,
    this.networkRpcMaxTimestampBehindMs,
    this.networkRpcMaxTimestampAheadMs,
    required this.networkRpcTimeoutMs,
    required this.networkRpcMaxRouteHopCount,
    this.networkDhtResolveNodeTimeoutMs,
    required this.networkDhtResolveNodeCount,
    required this.networkDhtResolveNodeFanout,
    required this.networkDhtMaxFindNodeCount,
    this.networkDhtGetValueTimeoutMs,
    required this.networkDhtGetValueCount,
    required this.networkDhtGetValueFanout,
    this.networkDhtSetValueTimeoutMs,
    required this.networkDhtSetValueCount,
    required this.networkDhtSetValueFanout,
    required this.networkDhtMinPeerCount,
    required this.networkDhtMinPeerRefreshTimeMs,
    required this.networkDhtValidateDialInfoReceiptTimeMs,
    required this.networkProtocolUdpEnabled,
    required this.networkProtocolUdpSocketPoolSize,
    required this.networkProtocolUdpListenAddress,
    this.networkProtocolUdpPublicAddress,
    required this.networkProtocolTcpConnect,
    required this.networkProtocolTcpListen,
    required this.networkProtocolTcpMaxConnections,
    required this.networkProtocolTcpListenAddress,
    this.networkProtocolTcpPublicAddress,
    required this.networkProtocolWsConnect,
    required this.networkProtocolWsListen,
    required this.networkProtocolWsMaxConnections,
    required this.networkProtocolWsListenAddress,
    required this.networkProtocolWsPath,
    this.networkProtocolWsUrl,
    required this.networkProtocolWssConnect,
    required this.networkProtocolWssMaxConnections,
    required this.networkLeasesMaxServerSignalLeases,
    required this.networkLeasesMaxServerRelayLeases,
    required this.networkLeasesMaxClientSignalLeases,
    required this.networkLeasesMaxClientRelayLeases,
  });

  String get json {
    return "";
  }

  factory VeilidConfig.fromJson(String json) {
    var parsed = jsonDecode(json);
    VeilidConfig({
      programName: parsed["program_name"],
      veilidNamespace: parsed["veilid_namespace"],
      apiLogLevel: veilidLogLevelFromJson(parsed["api_log_level"]),
      capabilitiesProtocolUdp: parsed["capabilities__protocol_udp"],
      capabilitiesProtocolConnectTcp: parsed["capabilities__protocol_connect_tcp"],
      capabilitiesProtocolAcceptTcp: parsed["capabilities__protocol_accept_tcp"],
      capabilitiesProtocolConnectWs: parsed["capabilities__protocol_connect_ws"],
      capabilitiesProtocolAcceptWs: parsed["capabilities__protocol_accept_ws"],
      capabilitiesProtocolConnectWss: parsed["capabilities__protocol_connect_wss"]
      // required this.capabilitiesProtocolAcceptWss,
      // required this.protectedStoreAllowInsecureFallback,
      // required this.protectedStoreAlwaysUseInsecureStorage,
      // required this.protectedStoreInsecureFallbackDirectory,
      // required this.protectedStoreDelete,
      // required this.tableStoreDirectory,
      // required this.tableStoreDelete,
      // required this.blockStoreDirectory,
      // required this.blockStoreDelete,
      // required this.networkMaxConnections,
      // required this.networkConnectionInitialTimeoutMs,
      // required this.networkNodeId,
      // required this.networkNodeIdSecret,
      // required this.networkBootstrap,
      // required this.networkUpnp,
      // required this.networkNatpmp,
      // required this.networkEnableLocalPeerScope,
      // required this.networkRestrictedNatRetries,
      // required this.networkRpcConcurrency,
      // required this.networkRpcQueueSize,
      // this.networkRpcMaxTimestampBehindMs,
      // this.networkRpcMaxTimestampAheadMs,
      // required this.networkRpcTimeoutMs,
      // required this.networkRpcMaxRouteHopCount,
      // this.networkDhtResolveNodeTimeoutMs,
      // required this.networkDhtResolveNodeCount,
      // required this.networkDhtResolveNodeFanout,
      // required this.networkDhtMaxFindNodeCount,
      // this.networkDhtGetValueTimeoutMs,
      // required this.networkDhtGetValueCount,
      // required this.networkDhtGetValueFanout,
      // this.networkDhtSetValueTimeoutMs,
      // required this.networkDhtSetValueCount,
      // required this.networkDhtSetValueFanout,
      // required this.networkDhtMinPeerCount,
      // required this.networkDhtMinPeerRefreshTimeMs,
      // required this.networkDhtValidateDialInfoReceiptTimeMs,
      // required this.networkProtocolUdpEnabled,
      // required this.networkProtocolUdpSocketPoolSize,
      // required this.networkProtocolUdpListenAddress,
      // this.networkProtocolUdpPublicAddress,
      // required this.networkProtocolTcpConnect,
      // required this.networkProtocolTcpListen,
      // required this.networkProtocolTcpMaxConnections,
      // required this.networkProtocolTcpListenAddress,
      // this.networkProtocolTcpPublicAddress,
      // required this.networkProtocolWsConnect,
      // required this.networkProtocolWsListen,
      // required this.networkProtocolWsMaxConnections,
      // required this.networkProtocolWsListenAddress,
      // required this.networkProtocolWsPath,
      // this.networkProtocolWsUrl,
      // required this.networkProtocolWssConnect,
      // required this.networkProtocolWssMaxConnections,
      // required this.networkLeasesMaxServerSignalLeases,
      // required this.networkLeasesMaxServerRelayLeases,
      // required this.networkLeasesMaxClientSignalLeases,
      // required this.networkLeasesMaxClientRelayLeases,
    })
      
  }
}

//////////////////////////////////////
/// VeilidUpdate

abstract class VeilidUpdate {
  factory VeilidUpdate.fromJson(String json) {
    var parsed = jsonDecode(json);
    switch (parsed["kind"]) {
      case "Log":
        {
          return VeilidUpdateLog(
              veilidLogLevelFromJson(parsed["log_level"]), parsed["message"]);
        }
      case "Attachment":
        {
          return VeilidUpdateAttachment(
              attachmentStateFromJson(parsed["state"]));
        }
      default:
        {
          throw VeilidAPIExceptionInternal(
              "Invalid VeilidAPIException type: ${parsed['kind']}");
        }
    }
  }
}

class VeilidUpdateLog implements VeilidUpdate {
  final VeilidLogLevel logLevel;
  final String message;
  //
  VeilidUpdateLog(this.logLevel, this.message);
}

class VeilidUpdateAttachment implements VeilidUpdate {
  final AttachmentState state;
  //
  VeilidUpdateAttachment(this.state);
}

//////////////////////////////////////
/// VeilidState

class VeilidState {
  final AttachmentState attachment;

  VeilidState(this.attachment);
}

//////////////////////////////////////
/// VeilidAPIException

abstract class VeilidAPIException implements Exception {
  factory VeilidAPIException.fromJson(String json) {
    var parsed = jsonDecode(json);
    switch (parsed["kind"]) {
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
          return VeilidAPIExceptionNodeNotFound(parsed["node_id"]);
        }
      case "NoDialInfo":
        {
          return VeilidAPIExceptionNoDialInfo(parsed["node_id"]);
        }
      case "Internal":
        {
          return VeilidAPIExceptionInternal(parsed["message"]);
        }
      case "Unimplemented":
        {
          return VeilidAPIExceptionUnimplemented(parsed["unimplemented"]);
        }
      case "ParseError":
        {
          return VeilidAPIExceptionParseError(
              parsed["message"], parsed["value"]);
        }
      case "InvalidArgument":
        {
          return VeilidAPIExceptionInvalidArgument(
              parsed["context"], parsed["argument"], parsed["value"]);
        }
      case "MissingArgument":
        {
          return VeilidAPIExceptionMissingArgument(
              parsed["context"], parsed["argument"]);
        }
      default:
        {
          throw VeilidAPIExceptionInternal(
              "Invalid VeilidAPIException type: ${parsed['kind']}");
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
  Future<void> changeApiLogLevel(VeilidLogLevel logLevel);
  Future<void> shutdownVeilidCore();
  String veilidVersionString();
  VeilidVersion veilidVersion();
}
