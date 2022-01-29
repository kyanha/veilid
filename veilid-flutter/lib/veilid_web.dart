import 'dart:async';
// In order to *not* need this ignore, consider extracting the "web" version
// of your plugin as a separate package, instead of inlining it in the same
// package as the core of your plugin.
// ignore: avoid_web_libraries_in_flutter
import 'dart:html' as html show window;

import 'package:flutter/services.dart';
import 'package:flutter_web_plugins/flutter_web_plugins.dart';

// xxx link in WASM version of veilid-flutter

/// A web implementation of the Veilid plugin.
class VeilidWeb {
  static void registerWith(Registrar registrar) {
    // final MethodChannel channel = MethodChannel(
    //   'veilid',
    //   const StandardMethodCodec(),
    //   registrar,
    // );

    // final pluginInstance = VeilidWeb();
    // channel.setMethodCallHandler(pluginInstance.handleMethodCall);
  }

  /// Handles method calls over the MethodChannel of this plugin.
  /// Note: Check the "federated" architecture for a new way of doing this:
  /// https://flutter.dev/go/federated-plugins
  Future<dynamic> handleMethodCall(MethodCall call) async {
    // switch (call.method) {
    //   case 'getPlatformVersion':
    //     return getPlatformVersion();
    //   default:
        throw PlatformException(
          code: 'Unimplemented',
          details: 'veilid for web doesn\'t implement \'${call.method}\'',
        );
    // }
  }

}
