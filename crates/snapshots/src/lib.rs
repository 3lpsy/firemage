//! Confined, bounded snapshot bundles containing memory, state and guest disks.
mod bundle;
mod files;
mod manifest;
pub use bundle::{inspect, pack, unpack};
pub use files::{create_private, hash_file, open_private};
pub use manifest::validate_manifest;
#[cfg(test)]
mod tests;
