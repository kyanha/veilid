#
# To learn more about a Podspec see http://guides.cocoapods.org/syntax/podspec.html.
# Run `pod lib lint veilid.podspec` to validate before publishing.
#
Pod::Spec.new do |s|
  s.name             = 'veilid'
  s.version          = '0.0.1'
  s.summary          = 'Veilid Network'
  s.description      = <<-DESC
Veilid Network Plugin
                       DESC
  s.homepage         = 'http://veilid.com'
  s.license          = 'LGPL-2.0-or-later OR MPL-2.0 OR (MIT AND BSD-3-Clause)'
  s.author           = { 'John Smith' => 'jsmith@example.com' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.11'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.swift_version = '5.0'

  require 'json'
  require 'pathname'
  cargo_target_dir = File.join(File.dirname(JSON.parse(`cargo locate-project`)['root']), 'target')

  s.script_phase = { 
    :name => 'Cargo Build', 
    :script => File.join(File.dirname(__dir__), 'rust', 'macos_build.sh'), 
    :execution_position => :before_compile
    #:output_files => [ File.join(cargo_target_dir, 'macos_lib', 'libveilid_flutter.dylib') ]
  }

end
