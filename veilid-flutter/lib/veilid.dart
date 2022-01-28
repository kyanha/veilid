
import 'dart:async';
import 'package:flutter/services.dart';
import 'package:veilid/bridge_generated.dart';

class Veilid {
  static const MethodChannel _channel = MethodChannel('veilid');

  static Future<String?> get platformVersion async {
    final String? version = await _channel.invokeMethod('getPlatformVersion');
    return version;
  }
}
