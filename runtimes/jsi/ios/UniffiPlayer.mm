/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#import "UniffiPlayer.h"

#import <Foundation/Foundation.h>

#include "ubrn_jsi_player.h"

#ifdef RCT_NEW_ARCH_ENABLED
namespace ubrn_player_ios {
using namespace facebook::react;

// The TurboModule subclass exists to give the install host function access to
// the CallInvoker; ObjCTurboModule does not expose it otherwise.
class JSI_EXPORT UniffiPlayerSpecJSI : public ObjCTurboModule {
public:
  UniffiPlayerSpecJSI(const ObjCTurboModule::InitParams &params);
  std::shared_ptr<CallInvoker> callInvoker;
};

static facebook::jsi::Value
__hostFunction_UniffiPlayer_install(facebook::jsi::Runtime &rt,
                                    TurboModule &turboModule,
                                    const facebook::jsi::Value *, size_t) {
  auto &tm = static_cast<UniffiPlayerSpecJSI &>(turboModule);
  // CocoaPods embeds a library's dynamic framework at
  // <app>/Frameworks/<name>.framework/<name>. privateFrameworksPath is nil
  // for bundles without a Frameworks dir, so guard against a NULL UTF8String.
  NSString *frameworksDir = [[NSBundle mainBundle] privateFrameworksPath];
  std::string frameworks = frameworksDir ? frameworksDir.UTF8String : "";
  ubrn::jsi_player::install(
      rt, tm.callInvoker, [frameworks](const std::string &name) {
        return frameworks + "/" + name + ".framework/" + name;
      });
  return facebook::jsi::Value(true);
}

UniffiPlayerSpecJSI::UniffiPlayerSpecJSI(
    const ObjCTurboModule::InitParams &params)
    : ObjCTurboModule(params), callInvoker(params.jsInvoker) {
  this->methodMap_["install"] =
      MethodMetadata{0, __hostFunction_UniffiPlayer_install};
}
} // namespace ubrn_player_ios
#endif

@implementation UniffiPlayer
RCT_EXPORT_MODULE(UniffiPlayer)

#ifdef RCT_NEW_ARCH_ENABLED
- (NSNumber *)install {
  // Codegen requires the selector; the JSI host function above is what runs.
  @throw [NSException exceptionWithName:@"UnreachableException"
                                 reason:@"install is served by UniffiPlayerSpecJSI"
                               userInfo:nil];
}

- (std::shared_ptr<facebook::react::TurboModule>)getTurboModule:
    (const facebook::react::ObjCTurboModule::InitParams &)params {
  return std::make_shared<ubrn_player_ios::UniffiPlayerSpecJSI>(params);
}
#endif

@end
