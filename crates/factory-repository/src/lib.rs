//! Observe FSD folders and imports, and project them onto visible graph boundaries.
pub mod graph;
pub mod projection;
mod tree;

pub use tree::{Entry, LAYERS, Snapshot, analyze};
