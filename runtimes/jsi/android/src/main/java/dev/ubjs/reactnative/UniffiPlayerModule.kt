/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
package dev.ubjs.reactnative

import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.module.annotations.ReactModule
import com.facebook.react.turbomodule.core.interfaces.CallInvokerHolder

@ReactModule(name = UniffiPlayerModule.NAME)
class UniffiPlayerModule(reactContext: ReactApplicationContext) :
  NativeUniffiPlayerSpec(reactContext) {

  override fun getName(): String = NAME

  // Implemented in cpp-adapter.cpp: installs `globalThis.uniffi` on the runtime.
  external fun nativeInstall(runtimePointer: Long, callInvoker: CallInvokerHolder): Boolean

  override fun install(): Boolean {
    val context = reactApplicationContext
    return nativeInstall(
      context.javaScriptContextHolder!!.get(),
      context.jsCallInvokerHolder!!,
    )
  }

  companion object {
    const val NAME = "UniffiPlayer"

    init {
      System.loadLibrary("ubrn_jsi_player")
    }
  }
}
