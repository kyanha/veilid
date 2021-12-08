#![forbid(unsafe_code)]
#![deny(clippy::all)]

mod client_api;
mod settings;

#[allow(clippy::all)]
pub mod veilid_client_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/veilid_client_capnp.rs"));
}

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        mod windows;

        fn main() -> windows_service::Result<(), String> {
            windows::main()
        }
    }
    else {
        mod unix;

        fn main() -> Result<(), String> {
            async_std::task::block_on(unix::main())
        }
    }
}
