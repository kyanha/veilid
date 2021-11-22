fn main() {
    ::capnpc::CompilerCommand::new()
        .file("../veilid-server/proto/veilid-client.capnp")
        .src_prefix("../veilid-server/")
        .run()
        .expect("compiling schema");
}
