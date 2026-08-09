fn main() {
    // Building for the host (e.g. an IDE running plain `cargo test -p kernel`)
    // dies later with a cryptic "unwinding panics are not supported without
    // std" — fail early with the actual fix instead.
    let target = std::env::var("TARGET").unwrap();
    if target != "x86_64-unknown-none" {
        panic!(
            "the kernel only builds for x86_64-unknown-none, not {target}.\n\
             use `cargo os` (run), `cargo ost` (test), or `cargo osd` (debug) — \n\
             or pass `--target x86_64-unknown-none` explicitly."
        );
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    println!("cargo:rustc-link-arg=-T{manifest_dir}/linker.ld");
    println!("cargo:rerun-if-changed=linker.ld");
}
