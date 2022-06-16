import 'veilid.dart';

import 'dart:html' as html;
import 'dart:js' as js;
import 'dart:js_util' as js_util;
import 'dart:async';
import 'dart:convert';

//////////////////////////////////////////////////////////

Veilid getVeilid() => VeilidJS();

Object wasm = js_util.getProperty(html.window, "veilid_wasm");

Future<T> _wrapApiPromise<T>(Object p) {
  return js_util.promiseToFuture(p).then((value) => value as T).catchError(
      (error) => Future<T>.error(
          VeilidAPIException.fromJson(jsonDecode(error as String))));
}

class VeilidJS implements Veilid {
  @override
  void configureVeilidPlatform(Map<String, dynamic> platformConfigJson) {
    var platformConfigJsonString =
        jsonEncode(platformConfigJson, toEncodable: veilidApiToEncodable);
    js_util.callMethod(
        wasm, "configure_veilid_platform", [platformConfigJsonString]);
  }

  @override
  Stream<VeilidUpdate> startupVeilidCore(VeilidConfig config) async* {
    var streamController = StreamController<VeilidUpdate>();
    updateCallback(String update) {
      var updateJson = jsonDecode(update);
      if (updateJson["kind"] == "Shutdown") {
        streamController.close();
      } else {
        var update = VeilidUpdate.fromJson(updateJson);
        streamController.add(update);
      }
    }

    await _wrapApiPromise(js_util.callMethod(wasm, "startup_veilid_core", [
      js.allowInterop(updateCallback),
      jsonEncode(config.json, toEncodable: veilidApiToEncodable)
    ]));
    yield* streamController.stream;
  }

  @override
  Future<VeilidState> getVeilidState() async {
    return VeilidState.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "get_veilid_state", []))));
  }

  @override
  Future<void> changeApiLogLevel(VeilidConfigLogLevel logLevel) {
    return _wrapApiPromise(js_util.callMethod(wasm, "change_api_log_level",
        [jsonEncode(logLevel.json, toEncodable: veilidApiToEncodable)]));
  }

  @override
  Future<void> shutdownVeilidCore() {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "shutdown_veilid_core", []));
  }

  @override
  Future<String> debug(String command) {
    return _wrapApiPromise(js_util.callMethod(wasm, "debug", [command]));
  }

  @override
  String veilidVersionString() {
    return js_util.callMethod(wasm, "veilid_version_string", []);
  }

  @override
  VeilidVersion veilidVersion() {
    var jsonVersion =
        jsonDecode(js_util.callMethod(wasm, "veilid_version", []));
    return VeilidVersion(
        jsonVersion["major"], jsonVersion["minor"], jsonVersion["patch"]);
  }
}
