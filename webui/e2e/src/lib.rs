//! Real-server desktop browser journeys, run through `just test-e2e`.
mod harness;
mod interaction;
pub use harness::*;

mod oidc;
mod process;
