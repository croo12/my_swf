#[path = "build_support/source_limits.rs"]
mod source_limits;

fn main() {
    let directory = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let directory = std::path::Path::new(&directory);
    // Watch the entire crate so adding an unreferenced .rs file also reruns the guard.
    println!("cargo:rerun-if-changed={}", directory.display());
    if let Err(error) = source_limits::check(directory) {
        panic!("{error}");
    }
}
