import 'veilid.dart';

import 'dart:js';
import 'dart:js_util';
import 'dart:async';
import 'dart:convert';
import 'package:flutter/services.dart' show NetworkAssetBundle;
import 'package:wasm_interop/wasm_interop.dart';
import 'package:mutex/mutex.dart';

//////////////////////////////////////////////////////////

Veilid getVeilid() => VeilidJS();

Instance? _wasmInstance;
final _wasmInstanceMutex = Mutex();

Future<Instance> getWasmInstance() async {
  await _wasmInstanceMutex.acquire();
  var _wi = _wasmInstance;
  if (_wi == null) {
    final bytes = await http???.get(Uri.parse("/wasm/veilid_wasm.wasm"));
    _wi = await Instance.fromBufferAsync(bytes.buffer);
    _wasmInstance = _wi;
  }
  _wasmInstanceMutex.release();
  return _wi;
}

class VeilidJS implements Veilid {
  @override
  Stream<VeilidUpdate> startupVeilidCore(VeilidConfig config) async* {
    var wasm = (await getWasmInstance());
    var streamController = StreamController<VeilidUpdate>();
    await promiseToFuture(
        wasm.functions["startup_veilid_core"]!.call((String update) {
      streamController.add(VeilidUpdate.fromJson(jsonDecode(update)));
    }, jsonEncode(config.json, toEncodable: veilidApiToEncodable)));
    yield* streamController.stream;
  }

  @override
  Future<VeilidState> getVeilidState() async {
    var wasm = (await getWasmInstance());
    return VeilidState.fromJson(jsonDecode(
        await promiseToFuture(wasm.functions["get_veilid_state"]!.call())));
  }

  @override
  Future<void> changeLogLevel(VeilidConfigLogLevel logLevel) async {
    var wasm = (await getWasmInstance());
    await promiseToFuture(wasm.functions["change_log_level"]!
        .call(jsonEncode(logLevel.json, toEncodable: veilidApiToEncodable)));
  }

  @override
  Future<void> shutdownVeilidCore() async {
    var wasm = (await getWasmInstance());
    await promiseToFuture(wasm.functions["shutdown_veilid_core"]!.call());
  }

  @override
  Future<String> debug(String command) async {
    var wasm = (await getWasmInstance());
    return await promiseToFuture(wasm.functions["debug"]!.call(command));
  }

  @override
  Future<String> veilidVersionString() async {
    var wasm = (await getWasmInstance());
    return await promiseToFuture(wasm.functions["debug"]!.call());
  }

  @override
  Future<VeilidVersion> veilidVersion() async {
    var wasm = (await getWasmInstance());
    var jsonVersion = jsonDecode(
        await promiseToFuture(wasm.functions["get_veilid_state"]!.call()));
    return VeilidVersion(
        jsonVersion["major"], jsonVersion["minor"], jsonVersion["patch"]);
  }
}
