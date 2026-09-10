use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-env-changed=FIREMAGE_WEBUI_EMBED_DIR");
    let explicit = env::var_os("FIREMAGE_WEBUI_EMBED_DIR");
    let root = explicit.clone().map(PathBuf::from).unwrap_or_else(|| {
        Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../dist/webui")
    });
    println!("cargo:rerun-if-changed={}", root.display());
    if explicit.is_some() && !root.join("index.html").is_file() {
        panic!("FIREMAGE_WEBUI_EMBED_DIR must contain a built index.html");
    }
    let mut files = Vec::new();
    if root.is_dir() {
        collect(&root, &root, &mut files);
    }
    files.sort();
    let entries = files
        .into_iter()
        .map(|(name, path)| {
            format!(
                "({name:?}, include_bytes!({:?}) as &'static [u8]),\n",
                path.to_str().unwrap()
            )
        })
        .collect::<String>();
    let output = format!("pub(super) static EMBEDDED: &[(&str, &[u8])] = &[\n{entries}];\n");
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("webui.rs"),
        output,
    )
    .unwrap();
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).expect("read webui bundle") {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            collect(root, &entry.path(), files);
        } else if kind.is_file() {
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            println!("cargo:rerun-if-changed={}", path.display());
            files.push((name, fs::canonicalize(path).unwrap()));
        } else {
            panic!("webui bundles must contain regular files and directories");
        }
    }
}
