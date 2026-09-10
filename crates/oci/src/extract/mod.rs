mod archive;
mod attributes;
mod filesystem;
mod layer;
mod metadata;
mod paths;
mod whiteouts;

pub(crate) use filesystem::has_file;
pub(crate) use layer::apply_layer;
pub(crate) use metadata::{ExtractionBudget, finalize};

#[cfg(test)]
mod tests;
