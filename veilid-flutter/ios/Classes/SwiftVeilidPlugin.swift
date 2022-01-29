import Flutter
import UIKit

public class SwiftVeilidPlugin: NSObject, FlutterPlugin {
  public static func register(with registrar: FlutterPluginRegistrar) {
    // No channel, FFI plugin
    print("dummy_value=\(dummy_method_to_enforce_bundling())");
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    // Noop
    result(nil)
  }
}
