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

@REM REM ensure winget is installed
@REM FOR %%X IN (winget.exe) DO (SET WINGET_FOUND=%%~$PATH:X)
@REM IF NOT DEFINED WINGET_FOUND (
@REM     echo Winget is not available in the path, ensure your version of Windows is new enough and has Winget installed from the Microsoft Store
@REM     echo https://docs.microsoft.com/en-us/windows/package-manager/winget/
@REM     goto end
@REM )
@REM echo [X] Winget is available in the path

@REM rem install cargo cbindgen
@REM cargo install cbindgen

@REM rem install dart ffigen
@REM call dart pub global activate ffigen

@REM rem install flutter_rust_bridge_codegen
@REM cargo install flutter_rust_bridge_codegen

@REM rem install just
@REM cargo install just

@REM rem ensure packages are installed
@REM winget install -e --id LLVM.LLVM --accept-package-agreements --accept-source-agreements

rem ensure windows is enabled in flutter
call flutter config --enable-windows-desktop --no-enable-android

rem turn off analytics
call flutter config --no-analytics
call dart --disable-analytics

call flutter doctor -v