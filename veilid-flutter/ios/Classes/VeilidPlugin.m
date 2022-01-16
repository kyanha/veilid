#import "VeilidPlugin.h"
#if __has_include(<veilid/veilid-Swift.h>)
#import <veilid/veilid-Swift.h>
#else
// Support project import fallback if the generated compatibility header
// is not copied when this plugin is created as a library.
// https://forums.swift.org/t/swift-static-libraries-dont-copy-generated-objective-c-header/19816
#import "veilid-Swift.h"
#endif

@implementation VeilidPlugin
+ (void)registerWithRegistrar:(NSObject<FlutterPluginRegistrar>*)registrar {
  [SwiftVeilidPlugin registerWithRegistrar:registrar];
}
@end
