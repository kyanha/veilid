import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:convert';

import 'package:ffi/ffi.dart';

import 'veilid.dart';

//////////////////////////////////////////////////////////

// Load the veilid_flutter library once
const _base = 'veilid_flutter';
final _path = Platform.isWindows
    ? '$_base.dll'
    : Platform.isMacOS
        ? 'lib$_base.dylib'
        : 'lib$_base.so';
late final _dylib =
    Platform.isIOS ? DynamicLibrary.process() : DynamicLibrary.open(_path);

// Linkage for initialization
typedef _dart_postCObject
    = NativeFunction<Int8 Function(Int64, Pointer<Dart_CObject>)>;
// fn free_string(s: *mut std::os::raw::c_char)
typedef _free_string_C = Void Function(Pointer<Utf8>);
typedef _free_string_Dart = void Function(Pointer<Utf8>);
// fn initialize_veilid_flutter(dart_post_c_object_ptr: ffi::DartPostCObjectFnType)
typedef _initializeVeilidFlutter_C = Void Function(Pointer<_dart_postCObject>);
typedef _initializeVeilidFlutter_Dart = void Function(
    Pointer<_dart_postCObject>);
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
class VeilidVersionFFI extends Struct {
  @Uint32()
  external int major;
  @Uint32()
  external int minor;
  @Uint32()
  external int patch;
}

typedef _veilid_version_C = VeilidVersionFFI Function();
typedef _veilid_version_Dart = VeilidVersionFFI Function();

// Async message types
const int MESSAGE_OK = 0;
const int MESSAGE_ERR = 1;
const int MESSAGE_OK_JSON = 2;
const int MESSAGE_ERR_JSON = 3;
const int MESSAGE_STREAM_ITEM = 4;
const int MESSAGE_STREAM_ITEM_JSON = 5;
const int MESSAGE_STREAM_ABORT = 6;
const int MESSAGE_STREAM_ABORT_JSON = 7;
const int MESSAGE_STREAM_CLOSE = 8;

// Interface factory for high level Veilid API
Veilid getVeilid() => VeilidFFI(_dylib);

// Parse handle async returns
Future<T> processSingleAsyncReturn<T>(Future<dynamic> future) async {
  return future.then((value) {
    final list = value as List<dynamic>;
    switch (list[0] as int) {
      case MESSAGE_OK:
        {
          if (list[1] != null) {
            throw VeilidAPIExceptionInternal(
                "Unexpected MESSAGE_OK value '${list[1]}' where null expected");
          }
          return list[1] as T;
        }
      case MESSAGE_ERR:
        {
          throw VeilidAPIExceptionInternal("Internal API Error: ${value[1]}");
        }
      case MESSAGE_OK_JSON:
        {
          var ret = jsonDecode(list[1] as String);
          if (ret != null) {
            throw VeilidAPIExceptionInternal(
                "Unexpected MESSAGE_OK_JSON value '$ret' where null expected");
          }
          return ret as T;
        }
      case MESSAGE_ERR_JSON:
        {
          throw VeilidAPIException.fromJson(value[1] as String);
        }
      default:
        {
          throw VeilidAPIExceptionInternal(
              "Unexpected async return message type: ${value[0]}");
        }
    }
  }).catchError((e) {
    // Wrap all other errors in VeilidAPIExceptionInternal
    throw VeilidAPIExceptionInternal(e.toString());
  }, test: (e) {
    // Pass errors that are already VeilidAPIException through without wrapping
    return e is! VeilidAPIException;
  });
}

Future<void> processSingleAsyncVoid(Future<dynamic> future) async {
  return future.then((value) {
    final list = value as List<dynamic>;
    switch (list[0] as int) {
      case MESSAGE_OK:
        {
          if (list[1] != null) {
            throw VeilidAPIExceptionInternal(
                "Unexpected MESSAGE_OK value '${list[1]}' where null expected");
          }
          return;
        }
      case MESSAGE_ERR:
        {
          throw VeilidAPIExceptionInternal("Internal API Error: ${value[1]}");
        }
      case MESSAGE_OK_JSON:
        {
          var ret = jsonDecode(list[1] as String);
          if (ret != null) {
            throw VeilidAPIExceptionInternal(
                "Unexpected MESSAGE_OK_JSON value '$ret' where null expected");
          }
          return;
        }
      case MESSAGE_ERR_JSON:
        {
          throw VeilidAPIException.fromJson(value[1] as String);
        }
      default:
        {
          throw VeilidAPIExceptionInternal(
              "Unexpected async return message type: ${value[0]}");
        }
    }
  }).catchError((e) {
    // Wrap all other errors in VeilidAPIExceptionInternal
    throw VeilidAPIExceptionInternal(e.toString());
  }, test: (e) {
    // Pass errors that are already VeilidAPIException through without wrapping
    return e is! VeilidAPIException;
  });
}

// FFI implementation of high level Veilid API
class VeilidFFI implements Veilid {
  // veilid_core shared library
  final DynamicLibrary _dylib;

  // Shared library functions
  final _free_string_Dart _freeString;
  final _startup_veilid_core_Dart _startupVeilidCore;
  final _get_veilid_state_Dart _getVeilidState;
  final _change_api_log_level_Dart _changeApiLogLevel;
  final _shutdown_veilid_core_Dart _shutdownVeilidCore;
  final _veilid_version_string_Dart _veilidVersionString;
  final _veilid_version_Dart _veilidVersion;

  VeilidFFI(DynamicLibrary dylib)
      : _dylib = dylib,
        _freeString = dylib
            .lookupFunction<_free_string_C, _free_string_Dart>('free_string'),
        _startupVeilidCore = dylib.lookupFunction<_startup_veilid_core_C,
            _startup_veilid_core_Dart>('startup_veilid_core'),
        _getVeilidState =
            dylib.lookupFunction<_get_veilid_state_C, _get_veilid_state_Dart>(
                'get_veilid_state'),
        _changeApiLogLevel = dylib.lookupFunction<_change_api_log_level_C,
            _change_api_log_level_Dart>('change_api_log_level'),
        _shutdownVeilidCore = dylib.lookupFunction<_shutdown_veilid_core_C,
            _shutdown_veilid_core_Dart>('shutdown_veilid_core'),
        _veilidVersionString = dylib.lookupFunction<_veilid_version_string_C,
            _veilid_version_string_Dart>('veilid_version_string'),
        _veilidVersion =
            dylib.lookupFunction<_veilid_version_C, _veilid_version_Dart>(
                'veilid_version') {
    // Get veilid_flutter initializer
    var initializeVeilidFlutter = _dylib.lookupFunction<
        _initializeVeilidFlutter_C,
        _initializeVeilidFlutter_Dart>('initialize_veilid_flutter');
    initializeVeilidFlutter(NativeApi.postCObject);
  }

  @override
  Stream<VeilidUpdate> startupVeilidCore(VeilidConfig config) async* {}

  @override
  Future<VeilidState> getVeilidState() async {
    final recv_port = ReceivePort("shutdown_veilid_core");
    final send_port = recv_port.sendPort;
    _shutdownVeilidCore(send_port.nativePort);
    processSingleAsyncReturn(recv_port.single);
  }

  @override
  Future<void> changeApiLogLevel(VeilidLogLevel logLevel) async {
    var nativeLogLevel = jsonEncode(logLevel).toNativeUtf8();
    final recv_port = ReceivePort("change_api_log_level");
    final send_port = recv_port.sendPort;
    _changeApiLogLevel(send_port.nativePort, nativeLogLevel);
    malloc.free(nativeLogLevel);
    processSingleAsyncVoid(recv_port.single);
  }

  @override
  Future<void> shutdownVeilidCore() async {
    final recv_port = ReceivePort("shutdown_veilid_core");
    final send_port = recv_port.sendPort;
    _shutdownVeilidCore(send_port.nativePort);
    processSingleAsyncVoid(recv_port.single);
  }

  @override
  String veilidVersionString() {
    final versionString = _veilidVersionString();
    String ret = versionString.toDartString();
    _freeString(versionString);
    return ret;
  }

  @override
  VeilidVersion veilidVersion() {
    final version = _veilidVersion();
    return VeilidVersion(
      version.major,
      version.minor,
      version.patch,
    );
  }
}
