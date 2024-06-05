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
  s.platform     = :ios, '9.0'
  s.source_files  = 'cpp/includes/*.{h,cpp,hpp}'
  s.dependency 'React'
end
