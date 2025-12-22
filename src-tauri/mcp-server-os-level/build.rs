fn main() {
    // Tell Cargo about our custom cfg values to avoid warnings
    println!("cargo::rustc-check-cfg=cfg(cargo_clippy)");
}