fn main() {
    println!("cargo:rerun-if-changed=../../schema/cafe.capnp");
    capnpc::CompilerCommand::new()
        .src_prefix("../../schema")
        .file("../../schema/cafe.capnp")
        .run()
        .expect("Install capnproto (capnp compiler)");
}
