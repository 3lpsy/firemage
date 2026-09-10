mod auth;
mod client;
mod download;
mod manifest;

pub(crate) use client::Registry;
pub(crate) use download::ensure_digest;
pub(crate) use manifest::architecture;

#[cfg(test)]
mod tests;
