//! Repository graph geometry and routing, independent of the desktop UI.
mod arrangement;
mod elk_routing;
mod embedded_router;
mod focus;
pub mod layout_engine;
mod routing;
mod scene;
pub mod worker;

pub use elk_routing::check_runtime;
pub use embedded_router::export_sources;
pub use focus::{Direction, inside, is_layer, relation};
pub use scene::{BoxItem, Route, Scene};

#[cfg(test)]
mod tests;
