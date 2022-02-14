import 'veilid.dart';

import 'dart:js';
import 'dart:async';
import 'dart:typed_data';

//////////////////////////////////////////////////////////

Veilid getVeilid() => VeilidJS();

class VeilidJS implements Veilid {
  Stream<VeilidUpdate> startupVeilidCore(VeilidConfig config) {
    throw UnimplementedError();
  }

  Future<VeilidState> getVeilidState() {
    throw UnimplementedError();
  }

  Future<void> changeApiLogLevel(VeilidConfigLogLevel logLevel) {
    throw UnimplementedError();
  }

  Future<void> shutdownVeilidCore() {
    throw UnimplementedError();
  }

  String veilidVersionString() {
    throw UnimplementedError();
  }

  VeilidVersion veilidVersion() {
    throw UnimplementedError();
  }
}
