use super::*;
use factory_core::repository_graph::Graph;
use std::{sync::mpsc, time::Instant};

struct Request {
    id: u64,
    graph: Graph,
    selected: String,
}
struct Response {
    id: u64,
    result: Result<Scene, String>,
    elapsed: Duration,
}
pub(super) struct Worker {
    send: mpsc::Sender<Request>,
    receive: mpsc::Receiver<Response>,
}
impl Worker {
    pub(super) fn new(ctx: egui::Context) -> Self {
        let (send, requests) = mpsc::channel::<Request>();
        let (responses, receive) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = requests.recv() {
                // Expansion clicks queued during a layout only need the newest graph.
                while let Ok(latest) = requests.try_recv() {
                    request = latest;
                }
                let start = Instant::now();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Scene::focused(request.graph, &request.selected)
                }))
                .unwrap_or_else(|_| Err("레이아웃 계산 중 오류가 발생했습니다.".into()));
                if responses
                    .send(Response {
                        id: request.id,
                        result,
                        elapsed: start.elapsed(),
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { send, receive }
    }
}

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
            let worker = self.worker.get_or_insert_with(|| Worker::new(ctx.clone()));
            if worker
                .send
                .send(Request {
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
            while let Ok(response) = worker.receive.try_recv() {
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
