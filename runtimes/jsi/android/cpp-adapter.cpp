/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include <ReactCommon/CallInvokerHolder.h>
#include <jni.h>
#include <jsi/jsi.h>

#include "ubrn_jsi_player.h"

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_ubjs_reactnative_UniffiPlayerModule_nativeInstall(
    JNIEnv *env, jobject /*thiz*/, jlong rtPtr,
    jobject callInvokerHolderJavaObj) {
  using JCallInvokerHolder = facebook::react::CallInvokerHolder;

  auto holderLocal = facebook::jni::make_local(callInvokerHolderJavaObj);
  auto holderRef = facebook::jni::static_ref_cast<JCallInvokerHolder::javaobject>(holderLocal);
  auto jsCallInvoker = holderRef->cthis()->getCallInvoker();
  auto *runtime = reinterpret_cast<facebook::jsi::Runtime *>(rtPtr);

  // A library's jniLibs land in the app's native-library directory, which the
  // linker already searches, so a soname is enough.
  ubrn::jsi_player::install(*runtime, jsCallInvoker,
                            [](const std::string &name) { return "lib" + name + ".so"; });
  return JNI_TRUE;
}
