use super::*;
use factory_layout::worker::{Request, Response};

impl View {
    pub(super) fn update_scene(
        &mut self,
        ctx: &egui::Context,
        snapshot: &Arc<Snapshot>,
        selected: &str,
    ) {
        let geometry_changed = self
            .snapshot
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, snapshot))
            || self.built_revision != self.revision;
        if geometry_changed || self.routed_selection != selected {
            if geometry_changed {
                self.scene = None;
            }
            self.error = None;
            self.pending = true;
            self.request += 1;
            let worker = self.worker.get_or_insert_with(|| {
                let ctx = ctx.clone();
                Worker::new(move || ctx.request_repaint())
            });
            if worker
                .submit(Request {
                    id: self.request,
                    graph: repository_graph::project(snapshot, &self.expanded),
                    selected: selected.to_owned(),
                })
                .is_err()
            {
                self.error = Some("레이아웃 작업을 시작하지 못했습니다.".into());
                self.pending = false;
                self.scene = Some(Scene::arrange(
                    repository_graph::project(snapshot, &self.expanded),
                    selected,
                ));
            }
            self.snapshot = Some(snapshot.clone());
            self.built_revision = self.revision;
        }
        self.routed_selection = selected.to_owned();
        while let Some(response) = self.worker.as_ref().and_then(Worker::try_recv) {
            self.apply_response(response, snapshot, selected);
        }
    }

    fn apply_response(&mut self, response: Response, snapshot: &Snapshot, selected: &str) {
        if response.id != self.request {
            return;
        }
        self.pending = false;
        self.elapsed = response.elapsed;
        match response.result {
            Ok(scene) => {
                self.scene = Some(scene);
                self.error = None;
            }
            Err(error) => {
                self.scene = Some(Scene::arrange(
                    repository_graph::project(snapshot, &self.expanded),
                    selected,
                ));
                self.error = Some(error);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_routing_failure(&mut self, snapshot: &Snapshot, selected: &str) {
        self.apply_response(
            Response {
                id: self.request,
                result: Err("Test routing failure".into()),
                elapsed: Duration::ZERO,
            },
            snapshot,
            selected,
        );
    }
}
