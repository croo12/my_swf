use super::*;
use factory_layout::worker::Request;

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
            }
            self.snapshot = Some(snapshot.clone());
            self.built_revision = self.revision;
        }
        self.routed_selection = selected.to_owned();
        if let Some(worker) = &self.worker {
            while let Some(response) = worker.try_recv() {
                if response.id != self.request {
                    continue;
                }
                self.pending = false;
                self.elapsed = response.elapsed;
                match response.result {
                    Ok(scene) => {
                        self.scene = Some(scene);
                        self.error = None;
                    }
                    Err(error) => {
                        self.scene = None;
                        self.error = Some(error);
                    }
                }
            }
        }
    }
}
