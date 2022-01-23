use cfg_if::*;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn resolve_llvm_path() -> Option<PathBuf> {
    let paths: Vec<PathBuf> =
        env::var_os("PATH").map(|paths| env::split_paths(&paths).collect())?;

    cfg_if! {
        if #[cfg(target_os="linux")] {
            // build host is linux

            // find clang
            let d = paths.iter().find_map(|p| {
                if p.join("clang").exists() {
                    if let Ok(real_clang_path) = fs::canonicalize(p.join("clang")) {
                        if let Some(llvmbindir) = real_clang_path.parent() {
                            if let Some(llvmdir) = llvmbindir.parent() {
                                return Some(llvmdir.to_owned());
                            }
                        }
                    }
                }
                None
            });

            d.or_else(|| {
                ["/usr/lib/llvm-13", "/usr/lib/llvm-12", "/usr/lib/llvm-11", "/usr/lib/llvm-10"].iter().map(Path::new).find_map(|p| if p.exists() { Some(p.to_owned()) } else { None } )
            })

        } else if #[cfg(target_os="macos")] {
            // build host is mac
            ["/opt/homebrew/opt/llvm", "/usr/local/homebrew/opt/llvm"].iter().map(Path::new).find_map(|p| if p.exists() { Some(p.to_owned()) } else { None } )
        } else {
            // anywhere else, just use the default paths
            llvm_path = None;
        }
    }
}

fn main() {
    //let out_dir = env::var_os("OUT_DIR").unwrap();
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();

    let input_path = Path::new(&manifest_dir).join("src").join("api.rs");
    let output_path = Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .join("lib")
        .join("bridge_generated.dart");
    let llvm_path = resolve_llvm_path();

    eprintln!("input_path: {:?}", input_path);
    eprintln!("output_path: {:?}", output_path);
    eprintln!("llvm_path: {:?}", llvm_path);

    let mut command = Command::new("flutter_rust_bridge_codegen");
    if let Some(llvm_path) = llvm_path {
        command.args([
            OsStr::new("--rust-input"),
            input_path.as_os_str(),
            OsStr::new("--dart-output"),
            output_path.as_os_str(),
            OsStr::new("--llvm-path"),
            llvm_path.as_os_str(),
        ]);
    } else {
        command.args([
            OsStr::new("--rust-input"),
            input_path.as_os_str(),
            OsStr::new("--dart-output"),
            output_path.as_os_str(),
        ]);
    }

    let mut child = command
        .spawn()
        .expect("flutter_rust_bridge_codegen did not execute correctly");
    child
        .wait()
        .expect("flutter_rust_bridge_codegen was not running");

    println!("cargo:rerun-if-changed={}", input_path.to_str().unwrap());
}
