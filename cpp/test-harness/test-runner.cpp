/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#ifndef _WIN32
#include <dlfcn.h>
#else
#include <windows.h>
#endif

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
  auto runtimeConfig =
      hermes::vm::RuntimeConfig::Builder().withIntl(false).build();

  // Create the Hermes runtime.
  auto runtime = facebook::hermes::makeHermesRuntime(runtimeConfig);
  std::shared_ptr<facebook::react::CallInvoker> invoker =
      std::make_shared<uniffi::testing::MyCallInvoker>(*runtime);

  // Register host functions.
  if (!loadNativeLibraries(*runtime, invoker, argc, argv))
    return 1;

  // Execute some JS.
  int status = 0;
  try {
    runtime->evaluateJavaScript(
        std::make_unique<facebook::jsi::StringBuffer>(std::move(*optCode)),
        jsPath);
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
