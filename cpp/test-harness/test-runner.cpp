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

static std::shared_ptr<facebook::jsi::Runtime> createRuntime() {
  auto runtimeConfig = ::hermes::vm::RuntimeConfig::Builder()
                           .withIntl(false)
                           .withMicrotaskQueue(true)
                           .build();
  return facebook::hermes::makeHermesRuntime(runtimeConfig);
}

static std::vector<RegisterNativesFN> loadNativeLibraryFunctions(int argc,
                                                                 char **argv) {
  std::vector<RegisterNativesFN> functions;
  for (int i = 2; i < argc; i++) {
    auto func = loadRegisterNatives(argv[i]);
    if (!func) {
      throw std::runtime_error("Failed to load native library");
    }
    functions.push_back(func);
  }
  return functions;
}

static void
registerNativeLibraries(facebook::jsi::Runtime &rt,
                        std::shared_ptr<facebook::react::CallInvoker> invoker,
                        const std::vector<RegisterNativesFN> &functions) {
  for (const auto &func : functions) {
    func(rt, invoker);
  }
}

static double currentTimeMillis() {
  auto now = std::chrono::steady_clock::now();
  return (double)std::chrono::duration_cast<std::chrono::milliseconds>(
             now.time_since_epoch())
      .count();
}

static int runEventLoop(facebook::jsi::Runtime &runtime,
                        std::shared_ptr<uniffi::testing::MyCallInvoker> invoker,
                        const std::string &jsCode, const char *jsPath) {
  try {
    facebook::jsi::Object helpers =
        runtime
            .evaluateJavaScript(
                std::make_unique<facebook::jsi::StringBuffer>(s_jslib),
                "timers.js.inc")
            .asObject(runtime);
    auto peekMacroTask = helpers.getPropertyAsFunction(runtime, "peek");
    auto runMacroTask = helpers.getPropertyAsFunction(runtime, "run");

    runMacroTask.call(runtime, currentTimeMillis());

    runtime.evaluateJavaScript(
        std::make_unique<facebook::jsi::StringBuffer>(jsCode), jsPath);
    invoker->drainTasks(runtime);
    runtime.drainMicrotasks();

    double nextTimeMs;
    while ((nextTimeMs = peekMacroTask.call(runtime).getNumber()) >= 0) {
      double duration = nextTimeMs - currentTimeMillis();
      if (duration > 0) {
        invoker->waitForTaskOrTimeout(duration);
      }
      invoker->drainTasks(runtime);
      runtime.drainMicrotasks();
      runMacroTask.call(runtime, currentTimeMillis());
      runtime.drainMicrotasks();
    }
    return 0;
  } catch (facebook::jsi::JSError &e) {
    std::cerr << "JS Exception: " << e.getStack() << std::endl;
    return 1;
  }
}

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

  try {
    auto nativeFunctions = loadNativeLibraryFunctions(argc, argv);

    // Run the test twice
    for (int i = 0; i < 2; i++) {
      std::cout << "Running iteration " << (i + 1) << std::endl;

      auto runtime = createRuntime();
      auto invoker = std::make_shared<uniffi::testing::MyCallInvoker>(*runtime);

      invoker->invokeAsync([i](facebook::jsi::Runtime &rt) {
        std::cout << "-- Starting the hermes event loop (iteration " << (i + 1)
                  << ")" << std::endl;
      });

      registerNativeLibraries(*runtime, invoker, nativeFunctions);

      int status = runEventLoop(*runtime, invoker, *optCode, jsPath);
      if (status != 0)
        return status;

      // Runtime will be destroyed here when shared_ptr goes out of scope
    }

    return 0;
  } catch (facebook::jsi::JSIException &e) {
    std::cerr << "JSI Exception: " << e.what() << std::endl;
    return 1;
  }
}
