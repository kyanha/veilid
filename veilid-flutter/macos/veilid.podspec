#
# To learn more about a Podspec see http://guides.cocoapods.org/syntax/podspec.html.
# Run `pod lib lint veilid.podspec` to validate before publishing.
#
Pod::Spec.new do |s|
  s.name             = 'veilid'
  s.version          = '0.0.1'
  s.summary          = 'Veilid Network'
  s.description      = <<-DESC
Veilid Network
                       DESC
  s.homepage         = 'http://example.com'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'Your Company' => 'email@example.com' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.11'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.swift_version = '5.0'

  s.script_phase = { :name => 'Cargo Build', :script => '../rust/macos_build.sh', :execution_position => :before_compile }

end
