@ECHO OFF
SETLOCAL

PUSHD %~dp0
SET ROOTDIR=%CD%
POPD

REM ensure flutter is installed
FOR %%X IN (flutter.bat) DO (SET FLUTTER_FOUND=%%~$PATH:X)
IF NOT DEFINED FLUTTER_FOUND (
    echo Flutter is not available in the path, install Flutter from here: https://docs.flutter.dev/get-started/install
    goto end
)
echo [X] Flutter is available in the path

REM ensure dart is installed
FOR %%X IN (dart.bat) DO (SET DART_FOUND=%%~$PATH:X)
IF NOT DEFINED DART_FOUND (
    echo Dart is not available in the path, check your environment variables and that Flutter is installed correctly
    goto end
)
echo [X] Dart is available in the path

REM ensure cargo is installed
FOR %%X IN (cargo.exe) DO (SET CARGO_FOUND=%%~$PATH:X)
IF NOT DEFINED CARGO_FOUND (
    echo Cargo is not available in the path, ensure Rust is installed correctly
    goto end
)
echo [X] Cargo is available in the path

REM ensure winget is installed
FOR %%X IN (winget.exe) DO (SET WINGET_FOUND=%%~$PATH:X)
IF NOT DEFINED WINGET_FOUND (
    echo Winget is not available in the path, ensure your version of Windows is new enough and has Winget installed from the Microsoft Store
    echo https://docs.microsoft.com/en-us/windows/package-manager/winget/
    goto end
)
echo [X] Winget is available in the path

rem install cargo cbindgen
cargo install cbindgen

rem install dart ffigen
call dart pub global activate ffigen

rem install flutter_rust_bridge_codegen
cargo install flutter_rust_bridge_codegen

rem install just
cargo install just

rem ensure packages are installed
winget install -e --id LLVM.LLVM --accept-package-agreements --accept-source-agreements

rem ensure windows is enabled in flutter
call flutter config --enable-windows-desktop --no-enable-android

rem turn off analytics
call flutter config --no-analytics
call dart --disable-analytics

call flutter doctor -v