use std::{env, fs, path::PathBuf};

#[path = "src/validate.rs"]
mod validation;

fn main() {
    println!("cargo:rerun-if-env-changed=FIREMAGE_GUEST_BIN_PATH");
    let path = env::var_os("FIREMAGE_GUEST_BIN_PATH")
        .map(PathBuf::from)
        .expect(
            "Build the embedded guest first: use `just build`, or run `just guest-build` and set \
         FIREMAGE_GUEST_BIN_PATH to its printed path before invoking Cargo directly",
        );
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(&path).expect("read FIREMAGE_GUEST_BIN_PATH");
    validation::validate(&bytes)
        .expect("FIREMAGE_GUEST_BIN_PATH must be a static Linux x86_64 ELF executable");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(output.join("firemage-guest"), bytes)
        .expect("copy validated guest into build output");
}
