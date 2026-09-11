//! Guest environment table, quick actions, and create/edit VM draft editor.
mod editor;
mod mutation;
mod panel;
mod variable_editor;
pub use editor::EnvironmentEditor;
pub use panel::Environment;

#[cfg(test)]
mod tests;
