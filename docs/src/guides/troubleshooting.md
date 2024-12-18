# Troubleshooting

Working with React Native can sometimes feel like casting spells: when it works, it's magic; but when you don't get the incantations in the right order, or the moon is in the wrong phase when retrograde to Mercury[^magick], then it can feel somewhat inscrutable.

This is not a comprehensive guide to debugging or troubleshooting your app or React Native setup.

These are things that contributors have encountered, and how they were resolved.

```admonish info title="Help wanted"
The most resiliant parts of the project is the generation of bindings between hermes and Rust.

The most fragile parts of the project are the interactions with the wider React Native project.

Currently, there is very little explicit React Native expertise in the project.

Please feel free to contribute to this page, either by organizing it, or by adding to it.

The best contributions would be pointing to other places on the internet with definitive advice.
```

[^magick]: I have no idea what I'm saying.

## iOS

### The build hangs shortly after `yarn example start`

Things I tried:

#### Running the app from Xcode

This workaround worked until I updated Xcode.

After updating Xcode, I saw build errors in Xcode (in the Report Navigator):

```sh
Run custom shell script 'Invoke Codgen'
/var/folders/sh/4_9lff8d37j8wvn1dn3gdb1r0000gp/T/SchemeScriptAction-2JFsLd.sh: line 2: npx: command not found
Exited with status code 127
```

I fixed this from the terminal before opening Xcode:

```sh
defaults write com.apple.dt.Xcode UseSanitizedBuildSystemEnvironment -bool NO
```

The problem here was that `npx` was being called during a Build Phase, but `npx` wasn't on the `PATH`.

#### A simulator isn't launched because more than one is available

```sh
success Successfully built the app
--- xcodebuild: WARNING: Using the first of multiple matching destinations:
{ platform:iOS, id:dvtdevice-DVTiPhonePlaceholder-iphoneos:placeholder, name:Any iOS Device }
{ platform:macOS, arch:arm64, variant:Designed for [iPad,iPhone], id:00006001-000A68400245801E, name:My Mac }
{ platform:iOS Simulator, id:dvtdevice-DVTiOSDeviceSimulatorPlaceholder-iphonesimulator:placeholder, name:Any iOS Simulator Device }
{ platform:iOS Simulator, id:4C1B86D9-3622-404F-83CA-410D9D909C7F, OS:17.0.1, name:iPad (10th generation) }
```

I have fixed this by launching a Simulator either from Spotlight (Cmd+Space, then typing Simulator) or by typing into a terminal:


```sh
udid=$(xcrun simctl list --json devices | jq -r '.devices[][] | select(.isAvailable == true) | .udid')
xcrun simctl boot "$udid"
```

#### A simulator isn't launched because it's trying to launch on a device

The error can be found by opening the `xcworkspace` file in Xcode with the `open` command.

```sh
error Signing for "RustOrBustExample" requires a development team. Select a development team in the Signing & Capabilities editor. (in target 'RustOrBustExample' from project 'RustOrBustExample')
error Failed to build ios project. "xcodebuild" exited with error code '65'. To debug build logs further, consider building your app with Xcode.app, by opening 'RustOrBustExample.xcworkspace'.
```

This can be fixed either by selecting a Simulator rather than a device (it's next to the Play button), or by following the error message and adding a development team in the Signings & Capabilities editor.

### Compiling for iOS gives an error `'UniffiCallInvoker.h' file not found`

We've seen this where there have been problems with the `*.podspec` file for the library.

- if the dependency on `uniffi-bindgen-react-native` isn't listed, it might be the podspec file isn't being generated at all.
- if the dependency on `uniffi-bindgen-react-native` is listed, check that the app's `Podfile` isn't also depending on `uniffi-bindgen-react-native`. Remove one of these dependencies.
- there may be multiple podspec files in your library, both of which depending on `uniffi-bindgen-react-native`. The name in the `ubrn.config.yaml` file can be deleted (where the podspec filename is derived from), as it should match the name derived from the `package.json` file.
