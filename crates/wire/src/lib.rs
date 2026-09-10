mod auth;
mod network;
mod vm;
pub use auth::*;
pub use network::*;
pub use vm::*;

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
