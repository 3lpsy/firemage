mod extract;
mod extract_sandbox;
mod image;
mod oci;
mod oci_init;
mod seed;
pub use extract::*;
pub use firemage_oci::{RegistryCredentials, RegistryOptions};
pub use image::*;
pub use seed::*;

pub async fn command(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(600),
        tokio::process::Command::new(program)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    anyhow::ensure!(
        output.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

mod guest_files;
pub use guest_files::*;
