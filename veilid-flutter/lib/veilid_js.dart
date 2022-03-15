import 'veilid.dart';

import 'dart:js';
import 'dart:async';
import 'dart:typed_data';

//////////////////////////////////////////////////////////

Veilid getVeilid() => VeilidJS();

class VeilidJS implements Veilid {
  @override
  Stream<VeilidUpdate> startupVeilidCore(VeilidConfig config) {
    throw UnimplementedError();
  }

  @override
  Future<VeilidState> getVeilidState() {
    throw UnimplementedError();
  }

  @override
  Future<void> changeLogLevel(VeilidConfigLogLevel logLevel) {
    throw UnimplementedError();
  }

  @override
  Future<void> shutdownVeilidCore() {
    throw UnimplementedError();
  }

  @override
  Future<String> debug(String command) {
    throw UnimplementedError();
  }

  @override
  String veilidVersionString() {
    throw UnimplementedError();
  }

  @override
  VeilidVersion veilidVersion() {
    throw UnimplementedError();
  }
}
