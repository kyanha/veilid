fn main() {
    ::capnpc::CompilerCommand::new()
        .file("proto/veilid.capnp")
        .run()
        .expect("compiling schema");
}
