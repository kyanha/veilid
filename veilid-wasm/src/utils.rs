use cfg_if::*;
use wasm_bindgen::prelude::*;
//use wasm_bindgen_futures::*;

cfg_if! {
    if #[cfg(feature = "wee_alloc")] {
        // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
        // allocator.
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn console_log(s: &str);

    #[wasm_bindgen]
    pub fn alert(s: &str);
}

pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

pub fn f64_try_to_unsigned<T>(f: f64) -> Result<T, ()>
where
    T: core::convert::TryFrom<u64>,
{
    let rf = f.floor();
    if rf < 0.0 {
        return Err(());
    }
    T::try_from(rf as u64).map_err(drop)
}
