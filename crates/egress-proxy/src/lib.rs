mod authority;
mod ca;
mod forwarding;
mod http;
mod manager;
mod resolve;

pub use manager::{EgressManager, Replacement};
mod transport;
pub use transport::validate_ca_pem;
mod upstream;
