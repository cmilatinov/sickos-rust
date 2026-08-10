fn main() {
    // Host builds must stay *checkable* — IDE project sync runs
    // `cargo check --workspace` with no --target and needs the kernel to
    // analyze cleanly. Anything past check (an actual host build/run/test,
    // e.g. plain `cargo test -p kernel`) dies at codegen/link; the warning
    // below is the breadcrumb explaining why.
    let target = std::env::var("TARGET").unwrap();
    if target != "x86_64-unknown-none" {
        println!(
            "cargo:warning=the kernel only runs on x86_64-unknown-none (building: {target}); \
             use `cargo os` (run), `cargo ost` (test), or `cargo osd` (debug)"
        );
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    println!("cargo:rustc-link-arg=-T{manifest_dir}/linker.ld");
    println!("cargo:rerun-if-changed=linker.ld");
}
