import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:oxidized/oxidized.dart';

import 'veilid_stub.dart'
  if (dart.library.io) 'veilid_ffi.dart'
  if (dart.library.js) 'veilid_js.dart';

//////////////////////////////////////////////////////////

enum AttachmentState {
  Detached,
  Attaching,
  AttachedWeak,
  AttachedGood,
  AttachedStrong,
  FullyAttached,
  OverAttached,
  Detaching,
}

enum VeilidLogLevel {
  Error,
  Warn,
  Info,
  Debug,
  Trace,
}

// VeilidVersion

class VeilidVersion {
  final int major;
  final int minor;
  final int patch;

  VeilidVersion({
    required this.major,
    required this.minor,
    required this.patch,
  });
}

// VeilidUpdate

abstract class VeilidUpdate {
  VeilidUpdateKind get kind;
}

class VeilidUpdateLog implements VeilidUpdate {
  final VeilidLogLevel logLevel;
  final String message;

  VeilidUpdateLog(this.logLevel, this.message);
}

class VeilidUpdateAttachment implements VeilidUpdate {
  final AttachmentState state;

  VeilidUpdateAttachment(this.state);
}

// VeilidState

class VeilidState {
  final AttachmentState attachment;

  VeilidState(this.attachment);
}



// Veilid singleton factory

abstract class Veilid {
  static Veilid _instance;

  static Veilid get instance {
    _instance ??= getVeilid();
    return _instance;
  }

  Stream<VeilidUpdate> startupVeilidCore(String config);
  Future<Result<VeilidState, VeilidAPIError>> getVeilidState();
  Future<Result<Unit, VeilidAPIError>> changeApiLogLevel(VeilidLogLevel logLevel);
  Future<Result<Unit, VeilidAPIError>> shutdownVeilidCore();
  String veilidVersionString();
  VeilidVersion veilidVersion();
}
