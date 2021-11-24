fn main() {
    ::capnpc::CompilerCommand::new()
        .file("../veilid-server/proto/veilid-client.capnp")
        .src_prefix("../veilid-server/")
        .run()
        .expect("compiling schema");
    #[cfg(unix)]
    {
        println!("cargo:rustc-link-lib=static=ncursesw");
    }
}
