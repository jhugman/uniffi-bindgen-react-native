/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <thread>
#ifndef _WIN32
#include <dlfcn.h>
#else
#include <windows.h>
#endif

/// The JS library that implements the setTimeout and setImmediate.
static const char *s_jslib =
#include "timers.js.inc"
    ;

#include "MyCallInvoker.h"
#include <ReactCommon/CallInvoker.h>
#include <hermes/hermes.h>

/// Read the contents of a file into a string.
static std::optional<std::string> readFile(const char *path) {
  std::ifstream fileStream(path);
  std::stringstream stringStream;

  if (fileStream) {
    stringStream << fileStream.rdbuf();
    fileStream.close();
  } else {
    // Handle error - file opening failed
    std::cerr << path << ": error opening file" << std::endl;
    return std::nullopt;
  }

  return stringStream.str();
}

/// The signature of the function that initializes the library.
typedef void (*RegisterNativesFN)(
    facebook::jsi::Runtime &rt,
    std::shared_ptr<facebook::react::CallInvoker> callInvoker);

#ifndef _WIN32
/// Load the library and return the "registerNatives()" function.
static RegisterNativesFN loadRegisterNatives(const char *libraryPath) {
  // Open the library.
  void *handle = dlopen(libraryPath, RTLD_LAZY);
  if (!handle) {
    std::cerr << "*** Cannot open library: " << dlerror() << '\n';
    return nullptr;
  }

  // Clear any existing error.
  dlerror();
  // Load the symbol (function).
  auto func = (RegisterNativesFN)dlsym(handle, "registerNatives");
  if (const char *dlsym_error = dlerror()) {
    std::cerr << "Cannot load symbol 'registerNatives': " << dlsym_error
              << '\n';
    dlclose(handle);
    return nullptr;
  }

  return func;
}
#else
/// Load the library and return the "registerNatives()" function.
static RegisterNativesFN loadRegisterNatives(const char *libraryPath) {
  // Load the library
  HMODULE hModule = LoadLibraryA(libraryPath);
  if (!hModule) {
    std::cerr << "Cannot open library: " << GetLastError() << '\n';
    return nullptr;
  }

  // Get the function address
  auto func = (RegisterNativesFN)GetProcAddress(hModule, "registerNatives");
  if (!func) {
    std::cerr << "Cannot load symbol 'registerNatives': " << GetLastError()
              << '\n';
    FreeLibrary(hModule);
    return nullptr;
  }

  return func;
}
#endif

/// Load all the libraries and call their "registerNatives()" function.
/// \return true if all libraries were loaded successfully.
static bool
loadNativeLibraries(facebook::jsi::Runtime &rt,
                    std::shared_ptr<facebook::react::CallInvoker> callInvoker,
                    int argc, char **argv) {
  try {
    for (int i = 2; i < argc; i++) {
      auto func = loadRegisterNatives(argv[i]);
      if (!func)
        return false;
      func(rt, callInvoker);
    }
  } catch (facebook::jsi::JSIException &e) {
    // Handle JSI exceptions here.
    std::cerr << "JSI Exception: " << e.what() << std::endl;
    return false;
  }
  return true;
}

static double currentTimeMillis() {
  auto now = std::chrono::steady_clock::now();
  return (double)std::chrono::duration_cast<std::chrono::milliseconds>(
             now.time_since_epoch())
      .count();
}

namespace jsi = facebook::jsi;
int main(int argc, char **argv) {
  // If no argument is provided, print usage and exit.
  if (argc < 2) {
    std::cout << "Usage: " << argv[0] << " <path-to-js-file> [<shared-lib>...]"
              << std::endl;
    return 1;
  }
  const char *jsPath = argv[1];

  // Read the file.
  auto optCode = readFile(jsPath);
  if (!optCode)
    return 1;

  // You can Customize the runtime config here.
  auto runtimeConfig = ::hermes::vm::RuntimeConfig::Builder()
                           .withIntl(false)
                           .withMicrotaskQueue(true)
                           .build();

  // Create the Hermes runtime.
  auto runtime = facebook::hermes::makeHermesRuntime(runtimeConfig);
  auto invoker = std::make_shared<uniffi::testing::MyCallInvoker>(*runtime);

  invoker->invokeAsync([](jsi::Runtime &rt) {
    std::cout << "-- Starting the hermes event loop" << std::endl;
  });

  // Register host functions.
  if (!loadNativeLibraries(*runtime, invoker, argc, argv))
    return 1;

  int status = 0;
  try {
    // Register event loop functions and obtain the runMicroTask() helper
    // function.
    jsi::Object helpers =
        runtime
            ->evaluateJavaScript(std::make_unique<jsi::StringBuffer>(s_jslib),
                                 "timers.js.inc")
            .asObject(*runtime);
    // `peek()` returns the time of the next pending task, or -1 if there is not
    // one.
    auto peekMacroTask = helpers.getPropertyAsFunction(*runtime, "peek");
    // `run(now: number)` looks for the next pending task and runs it.
    // `now` is the current time in milliseconds.
    // If no task is ready, the returns immediately.
    auto runMacroTask = helpers.getPropertyAsFunction(*runtime, "run");

    // There are no pending tasks, but we want to initialize the event loop
    // current time.
    runMacroTask.call(*runtime, currentTimeMillis());

    // Now we're ready to run the JS file.
    runtime->evaluateJavaScript(
        std::make_unique<jsi::StringBuffer>(std::move(*optCode)), jsPath);
    invoker->drainTasks(*runtime);
    runtime->drainMicrotasks();

    // This is the event loop. Loop while there are pending tasks.
    // Note that to use invokeAsync() you'll want to use setTimeout() to get
    // into this loop.
    double nextTimeMs;
    while ((nextTimeMs = peekMacroTask.call(*runtime).getNumber()) >= 0) {
      // If we have to, sleep until the next task is ready.
      double duration = nextTimeMs - currentTimeMillis();
      if (duration > 0) {
        invoker->waitForTaskOrTimeout(duration);
      }

      // Run ready tasks that came in from invoker.invokeAsync();
      invoker->drainTasks(*runtime);
      runtime->drainMicrotasks();

      // Then run the next timer task.
      runMacroTask.call(*runtime, currentTimeMillis());
      runtime->drainMicrotasks();
    }
  } catch (facebook::jsi::JSError &e) {
    // Handle JS exceptions here.
    std::cerr << "JS Exception: " << e.getStack() << std::endl;
    status = 1;
  } catch (facebook::jsi::JSIException &e) {
    // Handle JSI exceptions here.
    std::cerr << "JSI Exception: " << e.what() << std::endl;
    status = 1;
  }

  return status;
}
