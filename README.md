# Veilid

## Introduction

## Obtaining the source code

```shell
git clone --recurse-submodules git@gitlab.hackers.town:veilid/veilid.git
```

## Dependencies

### GNU/Linux

Development of Veilid on GNU/Linux requires a Debian variant such as Debian
itself, Ubuntu or Mint. Pull requests to support other distributions would be
welcome!

Development requires the Android SDK and NDK be installed.

You may decide to use Android Studio to maintain your Android dependencies. If
so, use the dependency manager within your IDE. If you do so, you may skip to
[Run Veilid setup script](#Run Veilid setup script).

* build-tools;30.0.3
* ndk;22.0.7026061
* cmake;3.22.1

#### Setup Dependencies using the CLI

Otherwise, you may choose to use Android `sdkmanager`. Follow the installation
instructions for `sdkmanager`
[here](https://developer.android.com/studio/command-line/sdkmanager), then use
the command line to install the requisite package versions:

```shell
sdkmanager --install "build-tools;30.0.3"
sdkmanager --install "ndk;22.0.7026061"
sdkmanager --install "cmake;3.22.1"
```

Export environment variables and add the Android SDK platform-tools directory to
your path.

```shell
cat << EOF >> ~/.profile 
export ANDROID_SDK_ROOT=<path to sdk>
export ANDROID_NDK_HOME=<path to ndk>
export PATH=${ANDROID_SDK_ROOT}/platform-tools"
```

#### Run Veilid setup script

Now you may run the Linux setup script to check your development environment and
pull the remaining Rust dependencies:

```shell
./setup_linux.sh
```

### macOS

**TODO**

### Windows

**TODO**

## Veilid Server

In order to run the `veilid-server` locally:

```shell
cd ./veilid-server
cargo run
```

In order to see what options are available:

```shell
cargo run -- --help
```

## Veilid CLI

In order to connect to your local `veilid-server`:

```shell
cd ./veilid-cli
cargo run
```

Similar to `veilid-server`, you may see CLI options by typing:

```shell
cargo run -- --help
```
