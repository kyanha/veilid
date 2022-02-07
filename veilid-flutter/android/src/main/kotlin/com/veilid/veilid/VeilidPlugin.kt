package com.veilid.veilid

import androidx.annotation.NonNull
import android.content.Context
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.MethodChannel.MethodCallHandler
import io.flutter.plugin.common.MethodChannel.Result

/** VeilidPlugin */
class VeilidPlugin: FlutterPlugin, MethodCallHandler {
  
  class object {
    {
      System.loadLibrary("veilid_flutter");
    }
  }

  native fun init_android(ctx: Context)

  override fun onAttachedToEngine(@NonNull flutterPluginBinding: FlutterPlugin.FlutterPluginBinding) {
    init_android(flutterPluginBinding.getApplicationContext())
  }

  override fun onMethodCall(@NonNull call: MethodCall, @NonNull result: Result) {
    result.notImplemented()
  }

  override fun onDetachedFromEngine(@NonNull binding: FlutterPlugin.FlutterPluginBinding) {
  }
}
