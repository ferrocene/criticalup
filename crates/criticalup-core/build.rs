fn main() {
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );

    // Only re-execute the build script when the build script itself changes.
    println!("cargo:rerun-if-changed=build.rs");
}
