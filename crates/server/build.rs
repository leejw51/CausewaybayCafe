fn main() {
    // SQLx embeds migrations; adding a migration must invalidate the compiled binary.
    println!("cargo:rerun-if-changed=migrations");
}
