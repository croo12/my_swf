mod cards;
mod comparison;
mod paint;
mod status;
mod viewport;
mod worker;

pub use comparison::run as compare;
use worker::Worker;

use crate::canvas::{self, ACCENT, BACKGROUND, BORDER, CARD, MUTED, TEXT};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use factory_core::{
    repository::{Entry, Snapshot},
    repository_graph,
};
use factory_layout::{Direction, Scene, inside, is_layer, relation};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

pub struct View {
    pub expanded: BTreeSet<String>,
    pub zoom: f32,
    snapshot: Option<Arc<Snapshot>>,
    scene: Option<Scene>,
    revision: u64,
    built_revision: u64,
    reveal: Option<String>,
    worker: Option<Worker>,
    request: u64,
    pending: bool,
    elapsed: Duration,
    error: Option<String>,
    fit_on_complete: bool,
    routed_selection: String,
}

impl Default for View {
    fn default() -> Self {
        Self {
            expanded: BTreeSet::new(),
            zoom: 0.85,
            snapshot: None,
            scene: None,
            revision: 0,
            built_revision: 0,
            reveal: Some(String::new()),
            worker: None,
            request: 0,
            pending: false,
            elapsed: Duration::ZERO,
            error: None,
            fit_on_complete: true,
            routed_selection: String::new(),
        }
    }
}

impl View {
    #[cfg(test)]
    pub(crate) fn test_scene(&self, selected: &str) -> Result<Option<&Scene>, &str> {
        if let Some(error) = &self.error {
            return Err(error);
        }
        Ok(
            if self.pending
                || self.built_revision != self.revision
                || self.routed_selection != selected
            {
                None
            } else {
                self.scene.as_ref()
            },
        )
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn layers(&mut self) {
        self.expanded.clear();
        self.revision += 1;
        self.reveal = Some(String::new());
        self.fit_on_complete = true;
    }
    pub fn slices(&mut self, snapshot: &Snapshot) {
        self.expanded = snapshot
            .tree
            .children
            .iter()
            .map(|node| node.path.clone())
            .collect();
        self.revision += 1;
        self.reveal = Some(String::new());
    }
    pub fn reveal(&mut self, tree: &Entry, path: &str) {
        repository_graph::reveal(tree, path, &mut self.expanded);
        self.revision += 1;
        self.reveal = Some(path.into());
    }
    fn toggle(&mut self, path: &str) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.into());
        }
        self.revision += 1;
        self.reveal = Some(path.into());
    }
    pub fn show(&mut self, ui: &mut egui::Ui, snapshot: &Arc<Snapshot>, selected: &mut String) {
        if is_layer(selected) {
            selected.clear();
        }
        let fit = status::controls(ui, selected, self.pending);
        self.update_scene(ui.ctx(), snapshot, selected);
        if let Some(error) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, error);
            return;
        }
        let Some(scene) = self.scene.as_ref() else {
            ui.label("중첩 그래프와 연결선을 배치하고 있습니다…");
            return;
        };
        if fit || self.fit_on_complete {
            self.zoom = ((ui.available_width() - 24.0) / scene.size.x)
                .clamp(canvas::GRAPH_ZOOM_MIN, canvas::GRAPH_ZOOM_MAX);
            self.fit_on_complete = false;
        }
        status::summary(ui, scene, selected, self.elapsed);
        self.viewport(ui, selected);
    }
}

pub fn kind_label(kind: &str) -> &str {
    match kind {
        "layer" => "LAYER",
        "slice" => "SLICE",
        "group" => "INDEX GROUP",
        "file" => "FILE",
        _ => "FOLDER",
    }
}
