VERSION 0.6

# Start with older Ubuntu to ensure GLIBC symbol versioning support for older linux
# Ensure we are using an amd64 platform because some of these targets use cross-platform tooling
FROM --platform amd64 ubuntu:16.04

# Choose where Rust ends up

# Install build prerequisites
deps-base:
    RUN apt-get -y update
    RUN apt-get install -y software-properties-common
    RUN add-apt-repository -y ppa:deadsnakes/ppa
    RUN apt-get -y update
    RUN apt-get install -y iproute2 curl build-essential cmake libssl-dev openssl file git pkg-config python3.8 python3.8-distutils python3.8-dev libdbus-1-dev libdbus-glib-1-dev libgirepository1.0-dev libcairo2-dev 
    RUN apt-get remove -y python3.5
    RUN curl https://bootstrap.pypa.io/get-pip.py | python3.8

# Install Cap'n Proto
deps-capnp:
    FROM +deps-base
    COPY scripts/earthly/install_capnproto.sh /
    RUN /bin/bash /install_capnproto.sh; rm /install_capnproto.sh

# Install Rust
deps-rust:
    FROM +deps-capnp
    ENV RUSTUP_HOME=/usr/local/rustup
    ENV CARGO_HOME=/usr/local/cargo
    ENV PATH=/usr/local/cargo/bin:$PATH
    RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -c clippy --no-modify-path --profile minimal 
    RUN chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
        rustup --version; \
        cargo --version; \
        rustc --version;
    # ARM64 Linux
    RUN rustup target add aarch64-unknown-linux-gnu
    # Android
    RUN rustup target add aarch64-linux-android
    RUN rustup target add armv7-linux-androideabi
    RUN rustup target add i686-linux-android
    RUN rustup target add x86_64-linux-android
    # WASM
    RUN rustup target add wasm32-unknown-unknown

# Install cross-platform tooling
deps-cross:
    FROM +deps-rust
    RUN apt-get install -y gcc-aarch64-linux-gnu 


# Install stub secrets daemon for keyring tests
deps-secretsd:
    FROM +deps-cross
    COPY scripts/earthly/secretsd /secretsd
    RUN pip install -r /secretsd/requirements.txt
    RUN pip install keyring
    RUN cp /secretsd/dbus/org.freedesktop.secrets.service /usr/share/dbus-1/services/org.freedesktop.secrets.service

# Clean up the apt cache to save space
deps:
    FROM +deps-secretsd
    RUN apt-get clean

code:
    FROM +deps
    COPY . .

# Clippy only
clippy:
    FROM +code
    RUN cargo clippy

# Build
build-linux-amd64:
    FROM +code
    RUN cargo build --target x86_64-unknown-linux-gnu --release
    SAVE ARTIFACT ./target/x86_64-unknown-linux-gnu AS LOCAL ./target/artifacts/x86_64-unknown-linux-gnu

build-linux-arm64:
    FROM +code
    RUN cargo build --target aarch64-unknown-linux-gnu --release
    SAVE ARTIFACT ./target/aarch64-unknown-linux-gnu AS LOCAL ./target/artifacts/aarch64-unknown-linux-gnu

# Unit tests
unit-tests-linux-amd64:
    FROM +code
    RUN cargo test --target x86_64-unknown-linux-gnu --release

unit-tests-linux-arm64:
    FROM +code
    RUN cargo test --target aarch64-unknown-linux-gnu --release
