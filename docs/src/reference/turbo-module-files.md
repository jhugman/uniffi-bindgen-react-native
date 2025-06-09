# Generating Turbo Module files to install the bindings

The bindings of the Rust library consist of several C++ files and several typescript files.

There is a host of smaller files that need to be configured with these namespaces, and with configuration from the [config YAML](config-yaml.md) file.

These include:

- For Javascript:
    - An `index.tsx` file, to call into the installation process, initialize the bindings for each namespace, and re-export the generated bindings for client code.
    - A Codegen file, to generates install methods from Javascript to Java and Objective C.
- For Android:
    - A `Package.java` and `Module.java` file, which receives the codegen'd install method calls, to get the Hermes `JavascriptRuntime` and `CallInvokerHolder` to pass it via JNI to
    - A `cpp-adapter.cpp` to receive the JNI calls, and converts those into `jsi::Runtime` and `react::CallInvoker` then calls into generic C++ install code.
- Generic C++ install code:
    - A turbo-module installation `.h` and `.cpp` which catches the calls from Android and iOS and registers the bindings C++ with the Hermes `jsi::Runtime`.
- For iOS:
    - a `Module.h` and `Module.mm` file which receives the codegen'd install method calls, and digs around to find the `jsi::Runtime` and `react::CallInvoker`. It then calls into the generic C++ install code.
- To build for iOS:
    - A podspec file to tell Xcode about the generated files, and the framework name/location of the compiled Rust library.
- To build for Android
    - A `CMakeLists.txt` file to configure the Android specific tool chain for all the generated C++ files.
    - The `build.gradle` file which tells keeps the codegen package name in-sync and configures `cmake`. (note to self, this could be done from within the `CMakeLists.txt` file).

An up-to-date list can be found in [`ubrn_cli/src/codegen/templates`](https://github.com/jhugman/uniffi-bindgen-react-native/tree/main/crates/ubrn_cli/src/codegen/templates).
