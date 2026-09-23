require "json"

package = JSON.parse(File.read(File.join(__dir__, "package.json")))

Pod::Spec.new do |s|
  s.name         = "UbjsReactNative"
  s.version      = package["version"]
  s.summary      = package["description"]
  s.homepage     = package["homepage"]
  s.license      = package["license"]
  s.authors      = package["author"]

  s.platforms    = { :ios => min_ios_version_supported }
  s.source       = { :git => package["repository"]["url"], :tag => "#{s.version}" }

  # The shim compiles here, against this app's jsi.h and CallInvoker.h, and
  # links the prebuilt Rust half. That half is a dynamic framework, so
  # CocoaPods embeds it in the app and signs it with the app's identity.
  s.source_files = "cpp/*.{h,cpp}", "include/*.h", "ios/*.{h,mm}"
  s.vendored_frameworks = "prebuilt/ios/UniffiRuntimeJsi.xcframework"
  s.pod_target_xcconfig = {
    "HEADER_SEARCH_PATHS" => "\"$(PODS_TARGET_SRCROOT)/cpp\" \"$(PODS_TARGET_SRCROOT)/include\"",
  }

  install_modules_dependencies(s)
end
