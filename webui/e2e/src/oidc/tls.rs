use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn certificates(directory: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    run(
        directory,
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=Browser test CA",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ],
    )?;
    run(
        directory,
        &[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-subj",
            "/CN=localhost",
            "-keyout",
            "provider.key",
            "-out",
            "provider.csr",
        ],
    )?;
    std::fs::write(
        directory.join("extensions.cnf"),
        "basicConstraints=critical,CA:FALSE\nsubjectAltName=IP:127.0.0.1\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n",
    )?;
    run(
        directory,
        &[
            "x509",
            "-req",
            "-in",
            "provider.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-days",
            "1",
            "-out",
            "provider.pem",
            "-extfile",
            "extensions.cnf",
        ],
    )?;
    Ok((
        directory.join("ca.pem"),
        directory.join("provider.pem"),
        directory.join("provider.key"),
    ))
}
fn run(directory: &Path, args: &[&str]) -> Result<()> {
    let result = Command::new("openssl")
        .args(args)
        .current_dir(directory)
        .output()?;
    anyhow::ensure!(
        result.status.success(),
        "OIDC fixture certificate generation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}
