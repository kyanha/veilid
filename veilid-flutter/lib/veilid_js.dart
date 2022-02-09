import 'veilid.dart';

import 'dart:js';
import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/services.dart';
import 'package:flutter/material.dart';

//////////////////////////////////////////////////////////

Veilid getVeilid() => VeilidJS();

class VeilidJS {
  Stream<VeilidUpdate> startupVeilidCore(Object? configCallback(String key)) {
    throw UnimplementedError();
  }

  Future<VeilidState> getVeilidState() {
    throw UnimplementedError();
  }

  Future<void> changeApiLogLevel(VeilidLogLevel logLevel) {
    throw UnimplementedError();
  }

  Future<void> shutdownVeilidCore() {
    throw UnimplementedError();
  }

  Future<String> veilidVersionString() {
    throw UnimplementedError();
  }

  Future<VeilidVersion> veilidVersion() {
    throw UnimplementedError();
  }
}
