mod auth;
mod network;
mod registry;
mod vm;
mod vm_config;
mod workload;
pub use auth::*;
pub use network::*;
pub use registry::*;
pub use vm::*;
pub use vm_config::*;
pub use workload::*;

pub fn ensure_name(value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "name must contain 1-64 letters, digits, underscores or hyphens"
    );
    Ok(())
}

mod secret;
pub use secret::*;

mod environment;
pub use environment::*;
pub use firemage_egress_policy::*;

mod boot;
pub use boot::*;

mod security;
pub use security::*;

mod kernel;
pub use kernel::*;

mod file_asset;
pub use file_asset::*;

mod terminal;
pub use terminal::*;

mod secret_attachment;
pub use secret_attachment::*;

mod guest_file;
pub use guest_file::*;

mod snapshot;
pub use snapshot::*;
mod egress_catalog;
pub use egress_catalog::*;
