use std::process::Command;
fn main() {
    let rustc = std::env::var_os("RUSTC").expect("Cargo supplies RUSTC");
    let output = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .expect("read sysroot");
    assert!(output.status.success(), "cannot locate compiler libraries");
    let root = String::from_utf8(output.stdout).expect("UTF-8 sysroot");
    let root = root.trim();
    println!("cargo:rustc-env=API_RUST_SYSROOT={root}");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        // rustc-dev's LLVM shared library is in the host sysroot lib directory.
        // rpath handles loading; native search is separately required by lld.
        println!("cargo:rustc-link-search=native={root}/lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{root}/lib");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
