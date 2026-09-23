// swift-tools-version:5.9
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Ships alongside the podspec for apps that consume React Native through SwiftPM.
// The consumer's React Native package supplies jsi.h and ReactCommon/CallInvoker.h.
// The binary target is a dynamic xcframework; SwiftPM embeds it.
import PackageDescription

let package = Package(
    name: "UbjsReactNative",
    platforms: [.iOS(.v15)],
    products: [
        .library(name: "UbjsReactNative", targets: ["UbjsReactNative"]),
    ],
    targets: [
        .binaryTarget(
            name: "UniffiRuntimeJsi",
            path: "prebuilt/ios/UniffiRuntimeJsi.xcframework"
        ),
        .target(
            name: "UbjsReactNative",
            dependencies: ["UniffiRuntimeJsi"],
            path: ".",
            sources: ["cpp", "ios"],
            publicHeadersPath: "include",
            cxxSettings: [
                .headerSearchPath("cpp"),
                .headerSearchPath("include"),
            ]
        ),
    ],
    cxxLanguageStandard: .cxx17
)
