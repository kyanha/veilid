use cfg_if::*;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

fn resolve_llvm_path() -> Option<PathBuf> {
    cfg_if! {
        if #[cfg(target_os="linux")] {
            // build host is linux
            let paths: Vec<PathBuf> =
                env::var_os("PATH").map(|paths| env::split_paths(&paths).collect())?;

            // find clang
            let d = paths.iter().find_map(|p| {
                if p.join("clang").exists() {
                    if let Ok(real_clang_path) = std::fs::canonicalize(p.join("clang")) {
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
            ["/usr/local/opt/llvm", "/opt/homebrew/opt/llvm", ].iter().map(Path::new).find_map(|p| if p.exists() { Some(p.to_owned()) } else { None } )
        } else {
            // anywhere else, just use the default paths
            None
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
    let c_path = Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .join("ios")
        .join("Classes")
        .join("bridge_generated.h");
    let llvm_path = resolve_llvm_path();

    //eprintln!("input_path: {:?}", input_path);
    //eprintln!("output_path: {:?}", output_path);
    //eprintln!("c_path: {:?}", c_path);
    //eprintln!("llvm_path: {:?}", llvm_path);

    let mut command = Command::new("flutter_rust_bridge_codegen");
    if let Some(llvm_path) = llvm_path {
        command.args([
            OsStr::new("--rust-input"),
            input_path.as_os_str(),
            OsStr::new("--dart-output"),
            output_path.as_os_str(),
            OsStr::new("--c-output"),
            c_path.as_os_str(),
            OsStr::new("--llvm-path"),
            llvm_path.as_os_str(),
        ]);
    } else {
        command.args([
            OsStr::new("--rust-input"),
            input_path.as_os_str(),
            OsStr::new("--dart-output"),
            output_path.as_os_str(),
            OsStr::new("--c-output"),
            c_path.as_os_str(),
        ]);
    }

    let mut child = command
        .spawn()
        .expect("flutter_rust_bridge_codegen did not execute correctly");
    child
        .wait()
        .expect("flutter_rust_bridge_codegen was not running");

    // Flutter pub get
    // Run: flutter pub get

    let mut command;
    cfg_if! {
        if #[cfg(target_os="windows")] {
            command = Command::new("cmd");
            command.args([
                OsStr::new("/c"),
                OsStr::new("flutter"),
                OsStr::new("pub"),
                OsStr::new("get"),
            ]);
        } else {
            command = Command::new("flutter");
            command.args([
                OsStr::new("pub"),
                OsStr::new("get"),
            ]);
        }
    }

    let mut child = command
        .spawn()
        .expect("'flutter pub get' did not execute correctly");
    child.wait().expect("'flutter pub get' was not running");

    // Build freezed
    // Run: flutter pub run build_runner build

    let mut command;
    cfg_if! {
        if #[cfg(target_os="windows")] {
            command = Command::new("cmd");
            command.args([
                OsStr::new("/c"),
                OsStr::new("flutter"),
                OsStr::new("pub"),
                OsStr::new("run"),
                OsStr::new("build_runner"),
                OsStr::new("build"),
            ]);
        } else {
            command = Command::new("flutter");
            command.args([
                OsStr::new("pub"),
                OsStr::new("run"),
                OsStr::new("build_runner"),
                OsStr::new("build"),
                OsStr::new("--delete-conflicting-outputs"),
            ]);
        }
    }

    let mut child = command
        .spawn()
        .expect("'flutter pub run build_runner build' did not execute correctly");
    child
        .wait()
        .expect("'flutter pub run build_runner build' was not running");
}
