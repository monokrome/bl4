use std::path::PathBuf;

fn main() {
    let oozlin_path = PathBuf::from("../../lib/oozlin");

    // Check if oozlin exists
    if !oozlin_path.exists() {
        panic!("oozlin submodule not found at {}. Run: git submodule update --init", oozlin_path.display());
    }

    // Compile oozlin C++ files plus our wrapper
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++11")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-unused-variable")
        .flag("-Wno-sign-compare")
        .flag("-Wno-parentheses")
        .flag("-Wno-unused-function")
        .flag("-Wno-sequence-point")
        .flag("-Wno-shift-negative-value")
        .flag("-O2")
        .include(&oozlin_path)
        .file(oozlin_path.join("kraken.cpp"))
        .file(oozlin_path.join("kraken_bits.cpp"))
        .file(oozlin_path.join("huff.cpp"))
        .file(oozlin_path.join("bitknit.cpp"))
        .file(oozlin_path.join("lzna.cpp"))
        .file(oozlin_path.join("mermaid.cpp"))
        .file(oozlin_path.join("leviathan.cpp"))
        .file(oozlin_path.join("utilities.cpp"))
        .file("ooz_wrapper.cpp")
        .compile("oozlin");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ooz_wrapper.cpp");
    println!("cargo:rerun-if-changed=../../lib/oozlin");
}
