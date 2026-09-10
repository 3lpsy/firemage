macro_rules! ensure { ($condition:expr, $($message:tt)*) => { if !$condition { return Err(anyhow::anyhow!($($message)*).into()); } }; }
pub(crate) use ensure;
mod app;
mod error;
mod identity;
mod login;
mod oidc;
mod resources;
mod tokens;
mod users;
pub use app::*;

mod assets;
mod browser;

mod management;

mod secrets;

mod egress;

mod socket;

mod kernels;

mod file_assets;
