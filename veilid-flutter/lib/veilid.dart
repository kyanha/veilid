import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:veilid/bridge_generated.dart';

const base = 'veilid_flutter';
final path = Platform.isWindows
    ? '$base.dll'
    : Platform.isMacOS
        ? 'lib$base.dylib'
        : 'lib$base.so';
late final dylib = Platform.isIOS ? DynamicLibrary.process() : DynamicLibrary.open(path);
late final veilidApi = VeilidFlutterImpl(dylib);

class Veilid {

  static VeilidFlutterImpl get api {
    if (veilidApi == null) {
      throw PlatformException(
        code: 'Library missing',
        details: 'veilid_core library could not be loaded dynamically',
      );
    }
    return veilidApi;
  }

}
