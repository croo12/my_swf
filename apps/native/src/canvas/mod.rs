mod card;
mod plan;
mod zoom;
pub use plan::plan;
pub use zoom::zoom_at_pointer;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use factory_core::{plan::Plan, repository::LAYERS};
use std::collections::HashMap;

pub const BACKGROUND: Color32 = Color32::from_rgb(10, 16, 27);
pub const PANEL: Color32 = Color32::from_rgb(15, 23, 37);
pub const CARD: Color32 = Color32::from_rgb(22, 33, 51);
pub const BORDER: Color32 = Color32::from_rgb(51, 66, 90);
pub const TEXT: Color32 = Color32::from_rgb(231, 237, 247);
pub const MUTED: Color32 = Color32::from_rgb(145, 161, 187);
pub const ACCENT: Color32 = Color32::from_rgb(155, 138, 251);
pub const GRAPH_ZOOM_MIN: f32 = 0.08;
pub const GRAPH_ZOOM_MAX: f32 = 1.3;

pub fn layer_color(path: &str) -> Color32 {
    match path.split('/').next().unwrap_or_default() {
        "app" => Color32::from_rgb(183, 149, 255),
        "pages" => Color32::from_rgb(105, 172, 255),
        "widgets" => Color32::from_rgb(92, 205, 224),
        "features" => Color32::from_rgb(112, 214, 169),
        "entities" => Color32::from_rgb(233, 190, 112),
        _ => Color32::from_rgb(183, 195, 215),
    }
}
