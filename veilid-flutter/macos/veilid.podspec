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

  s.script_phase = { 
    :name => 'Cargo Build', 
    :script => File.join(File.dirname(__dir__), 'rust', 'macos_build.sh'), 
    :execution_position => :before_compile,
    :output_files => [ File.join(File.dirname(__dir__), 'lib', 'libveilid_flutter.dylib') ]
  }

  # s.prepare_command = <<-CMD
  #   mkdir -p lib/Release/
  #   cp /dev/null lib/Release/libveilid_flutter.dylib
  # CMD

  # require 'json'
  # require 'pathname'
  # rust_dylib_absolute = File.join(File.dirname(JSON.parse(`cargo locate-project`)['root']), 'target', 'x86_64-apple-darwin', 'release', 'libveilid_flutter.dylib')
  # rust_dylib_relative = Pathname.new(rust_dylib_absolute).relative_path_from(Pathname.new(Dir.pwd)).to_s
  # require 'pp' 
  # print 'Rust dylib: '
  # pp rust_dylib_relative
  # s.vendored_libraries = 'lib/Release/libveilid_flutter.dylib'
  # s.libraries = [ 'veilid_flutter' ]

end
