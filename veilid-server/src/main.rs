#![forbid(unsafe_code)]

mod client_api;
mod settings;

pub mod veilid_client_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/veilid_client_capnp.rs"));
}

use cfg_if;

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
