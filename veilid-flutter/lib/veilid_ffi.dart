import 'dart:async';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:oxidized/oxidized.dart';

//////////////////////////////////////////////////////////

// Load the veilid_flutter library once
const _base = 'veilid_flutter';
final _path = Platform.isWindows
    ? '$_base.dll'
    : Platform.isMacOS
        ? 'lib$_base.dylib'
        : 'lib$_base.so';
late final _dylib = Platform.isIOS ? DynamicLibrary.process() : DynamicLibrary.open(_path);

// Linkage for initialization
typedef _dart_postCObject = NativeFunction<Int8 Function(Int64, Pointer<Dart_CObject>)>;
// fn free_string(s: *mut std::os::raw::c_char)
typedef _free_string_C = Void Function(Pointer<Utf8>);
typedef _free_string_Dart = void Function(Pointer<Utf8>);
// fn initialize_veilid_flutter(dart_post_c_object_ptr: ffi::DartPostCObjectFnType)
typedef _initializeVeilidFlutter_C = Void Function(Pointer<_dart_postCObject>);
typedef _initializeVeilidFlutter_Dart = void Function(Pointer<_dart_postCObject>);
// fn startup_veilid_core(port: i64, config: FfiStr)
typedef _startup_veilid_core_C = Void Function(Int64, Pointer<Utf8>);
typedef _startup_veilid_core_Dart = void Function(int, Pointer<Utf8>);
// fn get_veilid_state(port: i64)
typedef _get_veilid_state_C = Void Function(Int64);
typedef _get_veilid_state_Dart = void Function(int);
// fn change_api_log_level(port: i64, log_level: FfiStr)
typedef _change_api_log_level_C = Void Function(Int64, Pointer<Utf8>);
typedef _change_api_log_level_Dart = void Function(int, Pointer<Utf8>);
// fn shutdown_veilid_core(port: i64)
typedef _shutdown_veilid_core_C = Void Function(Int64);
typedef _shutdown_veilid_core_Dart = void Function(int);
// fn veilid_version_string() -> *mut c_char
typedef _veilid_version_string_C = Pointer<Utf8> Function();
typedef _veilid_version_string_Dart = Pointer<Utf8> Function();
// fn veilid_version() -> VeilidVersion
class VeilidVersion extends Struct {
  @Uint32()
  external int major;
  @Uint32()
  external int minor;
  @Uint32()
  external int patch;
}
typedef _veilid_version_C = VeilidVersion Function();
typedef _veilid_version_Dart = VeilidVersion Function();

// Interface factory for high level Veilid API
Veilid getVeilid() => VeilidFFI(_dylib);

// FFI implementation of high level Veilid API
class VeilidFFI {
  // veilid_core shared library
  final DynamicLibrary _dylib;

  // Shared library functions
  final _free_string_Dart _freeString;
  final _startup_veilid_core_Dart _startupVeilidCore;
  final _get_veilid_state_Dart _getVeilidState;
  final _change_api_log_level_Dart _changeApiLogLevel;
  final _shutdown_veilid_core_Dart _shutdownVeilidCore;
  final _veilid_version_string_Dart _veilidVersionString;
  final _veilid_version_Dat _veilidVersion;

  VeilidFFI(DynamicLibrary dylib): _dylib = dylib {
    var initializeVeilidFlutter = _dylib.lookupFunction<_initializeVeilidFlutter_C, _initializeVeilidFlutter_Dart>('initialize_veilid_flutter');
    initializeVeilidFlutter(NativeApi.postCObject);

    // Look up shared library functions
    _freeString = dylib.lookupFunction<_free_string_C, _free_string_Dart>('free_string');
    _startupVeilidCore = dylib.lookupFunction<_startup_veilid_core_C, _startup_veilid_core_Dart>('startup_veilid_core');
    _getveilidState = dylib.lookupFunction<_get_veilid_state_C, _get_veilid_state_Dart>('get_veilid_state');
    _changeApiLogLevel = dylib.lookupFunction<_change_api_log_level_C, _change_api_log_level_Dart>('change_api_log_level');
    _shutdownVeilidCore = dylib.lookupFunction<_shutdown_veilid_core_C, _shutdown_veilid_core_Dart>('shutdown_veilid_core');
    _veilidVersionString = dylib.lookupFunction<_veilid_version_string_C, _veilid_version_string_Dart>('veilid_version_string');
    _veilidVersion = dylib.lookupFunction<_veilid_version_C, _veilid_version_Dart>('veilid_version');
  }

  Stream<VeilidUpdate> startupVeilidCore(String config);
  Future<Result<VeilidState, VeilidAPIError>> getVeilidState();
  Future<Result<Unit, VeilidAPIError>> changeApiLogLevel(VeilidLogLevel logLevel);
  Future<Result<Unit, VeilidAPIError>> shutdownVeilidCore() async {
    // xxx continue here
  }

  String veilidVersionString() {
    final version_string = _veilidVersionString();
    String ret = version_string.toDartString();
    _freeString(version_string);
    return version_string;
  }

  VeilidVersion veilidVersion() {
    return _veilidVersion();
  }

}
