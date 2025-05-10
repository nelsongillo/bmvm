use std::env;
use std::path::PathBuf;

fn main() {
    // ensure the build script is re-run if it changes
    println!("cargo:rerun-if-changed=build.rs");

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // add the out directory to the link search path
    println!("cargo:rustc-link-search={}", out.display());

    // copy all link scripts to the out directory
    for entry in std::fs::read_dir("link").unwrap() {
        let entry = entry.unwrap();
        let src = entry.path();
        let dst = out.join(&src.file_name().unwrap());

        println!("cargo:rerun-if-changed={}", &src.display());
        std::fs::copy(&src, &dst).unwrap();
    }
}
