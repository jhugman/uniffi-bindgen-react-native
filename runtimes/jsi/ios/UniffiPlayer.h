/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#ifdef RCT_NEW_ARCH_ENABLED
#import "UniffiPlayerSpec.h"

@interface UniffiPlayer : NSObject <NativeUniffiPlayerSpec>
#else
#import <React/RCTBridgeModule.h>

@interface UniffiPlayer : NSObject <RCTBridgeModule>
#endif

@end
