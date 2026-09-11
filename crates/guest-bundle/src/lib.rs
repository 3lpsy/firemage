//! Statically linked guest executable included in host builds.

mod validate;
pub use validate::validate;

/// The validated guest executable. Callers inject it only when Web Terminal is enabled.
pub fn bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/firemage-guest"))
}

#[cfg(test)]
mod tests;
