require 'json'
package = JSON.parse(File.read(File.join(__dir__, 'package.json')))

Pod::Spec.new do |s|
  s.name             = package['name']
  s.version          = package['version']
  s.summary          = package['description']
  s.homepage         = package['homepage']
  s.license          = { :type => package['license'], :file => 'LICENSE' }
  s.author           = { package['author']['name'] => package['author']['email'] }
  s.source           = { :git => package['repository']['url'], :tag => s.version.to_s }
  s.platform         = :ios, '13.0'
  s.source_files     = 'cpp/includes/*.{h,cpp,hpp}'
  s.swift_versions   = '4.0'
  s.pod_target_xcconfig = {
    'SWIFT_OPTIMIZATION_LEVEL' => '-Onone',
  }
  s.dependency 'React-Core'
end
