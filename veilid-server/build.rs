fn main() {
    ::capnpc::CompilerCommand::new()
        .file("proto/veilid-client.capnp")
        .run()
        .expect("compiling schema");
}
