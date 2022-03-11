import 'package:veilid/veilid.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;

Future<VeilidConfig> getDefaultVeilidConfig() async {
  return VeilidConfig(
    programName: "Veilid Plugin Test",
    namespace: "",
    apiLogLevel: VeilidConfigLogLevel.info,
    capabilities: VeilidConfigCapabilities(
      protocolUDP: !kIsWeb,
      protocolConnectTCP: !kIsWeb,
      protocolAcceptTCP: !kIsWeb,
      protocolConnectWS: true,
      protocolAcceptWS: !kIsWeb,
      protocolConnectWSS: true,
      protocolAcceptWSS: false,
    ),
    protectedStore: VeilidConfigProtectedStore(
      allowInsecureFallback: false,
      alwaysUseInsecureStorage: false,
      insecureFallbackDirectory: "",
      delete: false,
    ),
    tableStore: VeilidConfigTableStore(
      directory: p.join((await getApplicationSupportDirectory()).absolute.path,
          "table_store"),
      delete: false,
    ),
    blockStore: VeilidConfigBlockStore(
      directory: p.join((await getApplicationSupportDirectory()).absolute.path,
          "block_store"),
      delete: false,
    ),
    network: VeilidConfigNetwork(
        maxConnections: 16,
        connectionInitialTimeoutMs: 2000,
        nodeId: "",
        nodeIdSecret: "",
        bootstrap: [],
        rpc: VeilidConfigRPC(
          concurrency: 0,
          queueSize: 1024,
          maxTimestampBehindMs: 10000,
          maxTimestampAheadMs: 10000,
          timeoutMs: 10000,
          maxRouteHopCount: 7,
        ),
        dht: VeilidConfigDHT(
          resolveNodeTimeoutMs: null,
          resolveNodeCount: 20,
          resolveNodeFanout: 3,
          maxFindNodeCount: 20,
          getValueTimeoutMs: null,
          getValueCount: 20,
          getValueFanout: 3,
          setValueTimeoutMs: null,
          setValueCount: 20,
          setValueFanout: 5,
          minPeerCount: 20,
          minPeerRefreshTimeMs: 2000,
          validateDialInfoReceiptTimeMs: 5000,
        ),
        upnp: true,
        natpmp: true,
        enableLocalPeerScope: false,
        restrictedNatRetries: 3,
        tls: VeilidConfigTLS(
          certificatePath: "",
          privateKeyPath: "",
          connectionInitialTimeoutMs: 2000,
        ),
        application: VeilidConfigApplication(
            https: VeilidConfigHTTPS(
              enabled: false,
              listenAddress: "",
              path: "",
              url: null,
            ),
            http: VeilidConfigHTTP(
              enabled: false,
              listenAddress: "",
              path: "",
              url: null,
            )),
        protocol: VeilidConfigProtocol(
          udp: VeilidConfigUDP(
            enabled: !kIsWeb,
            socketPoolSize: 0,
            listenAddress: "",
            publicAddress: null,
          ),
          tcp: VeilidConfigTCP(
            connect: !kIsWeb,
            listen: !kIsWeb,
            maxConnections: 32,
            listenAddress: "",
            publicAddress: null,
          ),
          ws: VeilidConfigWS(
            connect: true,
            listen: !kIsWeb,
            maxConnections: 16,
            listenAddress: "",
            path: "ws",
            url: null,
          ),
          wss: VeilidConfigWSS(
            connect: true,
            listen: false,
            maxConnections: 16,
            listenAddress: "",
            path: "ws",
            url: null,
          ),
        ),
        leases: VeilidConfigLeases(
          maxServerSignalLeases: 256,
          maxServerRelayLeases: 8,
          maxClientSignalLeases: 2,
          maxClientRelayLeases: 2,
        )),
  );
}
