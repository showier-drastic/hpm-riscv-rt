fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=link.x");

    // copy link.x to the output directory
    std::fs::copy("link.x", format!("{}/link.x", out_dir)).unwrap();

    // add the linker script to the build
    println!("cargo:rustc-link-search={}", out_dir);
}
