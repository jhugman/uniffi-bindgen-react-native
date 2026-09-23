/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#include <chrono>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <thread>
#ifndef _WIN32
#include <dlfcn.h>
#include <unistd.h>
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
#include <jsi/instrumentation.h>

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
  // Cap the GC heap well below Hermes' 3 GB default. A real RN host runs Hermes
  // with a bounded heap so the GC collects under memory pressure; with the
  // default 3 GB cap a tight *synchronous* loop (e.g. lifting a deep recursive
  // structure x100) lets transient garbage balloon for seconds before Hermes
  // collects — and the OS can SIGKILL the process first. The event-loop GC
  // can't help a sync loop (it never yields), so the bound has to live here.
  // 1 GB is ample headroom over any fixture's legitimate live set.
  auto gcConfig = ::hermes::vm::GCConfig::Builder()
                      .withMaxHeapSize(1u << 30) // 1 GB
                      .build();
  auto runtimeConfig = ::hermes::vm::RuntimeConfig::Builder()
                           .withIntl(false)
                           .withMicrotaskQueue(true)
                           .withGCConfig(gcConfig)
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

static void installPerformanceNow(facebook::jsi::Runtime &runtime) {
  auto fn = facebook::jsi::Function::createFromHostFunction(
      runtime, facebook::jsi::PropNameID::forAscii(runtime, "__performanceNow"),
      0,
      [](facebook::jsi::Runtime &rt, const facebook::jsi::Value &,
         const facebook::jsi::Value *, size_t) -> facebook::jsi::Value {
        auto now = std::chrono::steady_clock::now().time_since_epoch();
        double ms = std::chrono::duration<double, std::milli>(now).count();
        return facebook::jsi::Value(ms);
      });
  runtime.global().setProperty(runtime, "__performanceNow", fn);
}

static void installHeapInfo(facebook::jsi::Runtime &runtime) {
  auto fn = facebook::jsi::Function::createFromHostFunction(
      runtime, facebook::jsi::PropNameID::forAscii(runtime, "__hermesHeapInfo"),
      0,
      [](facebook::jsi::Runtime &rt, const facebook::jsi::Value &,
         const facebook::jsi::Value *, size_t) -> facebook::jsi::Value {
        auto info =
            rt.instrumentation().getHeapInfo(/*includeExpensive=*/false);
        facebook::jsi::Object out(rt);
        for (const auto &kv : info) {
          out.setProperty(rt, kv.first.c_str(),
                          facebook::jsi::Value(static_cast<double>(kv.second)));
        }
        return facebook::jsi::Value(rt, out);
      });
  runtime.global().setProperty(runtime, "__hermesHeapInfo", fn);
}

static void installGc(facebook::jsi::Runtime &runtime) {
  auto fn = facebook::jsi::Function::createFromHostFunction(
      runtime, facebook::jsi::PropNameID::forAscii(runtime, "__hermesGc"), 0,
      [](facebook::jsi::Runtime &rt, const facebook::jsi::Value &,
         const facebook::jsi::Value *, size_t) -> facebook::jsi::Value {
        rt.instrumentation().collectGarbage("test-runner");
        return facebook::jsi::Value::undefined();
      });
  runtime.global().setProperty(runtime, "__hermesGc", fn);
}

static int runEventLoop(facebook::jsi::Runtime &runtime,
                        std::shared_ptr<uniffi::testing::MyCallInvoker> invoker,
                        const std::string &jsCode, const char *jsPath) {
  try {
    installPerformanceNow(runtime);
    installHeapInfo(runtime);
    installGc(runtime);
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
    // As the React Native *host* stand-in, the test-runner owns GC scheduling.
    // A real RN Hermes collects during async work (idle callbacks / heap
    // pressure); this bare event loop does not, so transient per-await garbage
    // (promises, closures, handle-map churn) accumulates unbounded inside a
    // tight `for (await ...)` loop — allocation cost then grows linearly until
    // nothing ever collects it. Collect when the live heap has grown past a
    // threshold since the last collection: cheap when async is idle, and only
    // fires under real allocation pressure — a proxy for production GC
    // behaviour.
    auto allocatedBytes = [&runtime]() -> int64_t {
      const auto info =
          runtime.instrumentation().getHeapInfo(/*includeExpensive=*/false);
      const auto it = info.find("hermes_allocatedBytes");
      return it != info.end() ? it->second : 0;
    };
    constexpr int64_t kGcGrowthThreshold = 4 * 1024 * 1024; // 4 MB
    int64_t lastGcBytes = allocatedBytes();
    uint64_t loopCount = 0;
    while ((nextTimeMs = peekMacroTask.call(runtime).getNumber()) >= 0) {
      double duration = nextTimeMs - currentTimeMillis();
      if (duration > 0) {
        invoker->waitForTaskOrTimeout(duration);
      }
      invoker->drainTasks(runtime);
      runtime.drainMicrotasks();
      runMacroTask.call(runtime, currentTimeMillis());
      runtime.drainMicrotasks();
      // Check growth periodically (getHeapInfo is cheap but not free).
      if ((++loopCount & 0x3F) == 0 &&
          allocatedBytes() - lastGcBytes > kGcGrowthThreshold) {
        runtime.instrumentation().collectGarbage("test-runner: heap growth");
        lastGcBytes = allocatedBytes();
      }
    }
    return 0;
  } catch (facebook::jsi::JSError &e) {
    std::cerr << "JS Exception: " << e.getStack() << std::endl;
    return 1;
  }
}

// Exit if our parent process dies. The test-runner is spawned by the Rust test
// harness and can run for many minutes (e.g. the benchmark fixture). If the
// harness is interrupted (Ctrl-C, SIGKILL, an IDE stopping the run), the child
// is reparented to init/launchd and keeps grinding at ~100% CPU as an orphan —
// repeated interrupted runs then pile up CPU-pegged orphans that starve later
// runs. A background watchdog polls getppid(): once it changes, the original
// launcher is gone, so we _Exit immediately. This runs on its own thread, so it
// fires even while the JS thread is blocked in a long synchronous evaluate.
static void startParentDeathWatchdog() {
#ifndef _WIN32
  const pid_t initialParent = getppid();
  std::thread([initialParent]() {
    for (;;) {
      std::this_thread::sleep_for(std::chrono::milliseconds(250));
      if (getppid() != initialParent) {
        // Orphaned: skip all teardown (another thread may be mid-Hermes) and
        // go.
        std::_Exit(2);
      }
    }
  }).detach();
#endif
}

int main(int argc, char **argv) {
  startParentDeathWatchdog();

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
