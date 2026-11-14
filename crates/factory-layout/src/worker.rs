//! Coalescing background layout calculations with an event-loop-independent wake callback.
use crate::Scene;
use factory_repository::projection::Graph;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

pub struct Request {
    pub id: u64,
    pub graph: Graph,
    pub selected: String,
}
pub struct Response {
    pub id: u64,
    pub result: Result<Scene, String>,
    pub elapsed: Duration,
}
pub struct Worker {
    send: mpsc::Sender<Request>,
    receive: mpsc::Receiver<Response>,
}
impl Worker {
    pub fn new(notify: impl Fn() + Send + 'static) -> Self {
        Self::spawn(notify, Scene::focused)
    }

    fn spawn(
        notify: impl Fn() + Send + 'static,
        mut layout: impl FnMut(Graph, &str) -> Result<Scene, String> + Send + 'static,
    ) -> Self {
        let (send, requests) = mpsc::channel::<Request>();
        let (responses, receive) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = requests.recv() {
                // Only the newest graph queued during an active calculation is needed.
                while let Ok(latest) = requests.try_recv() {
                    request = latest;
                }
                let start = Instant::now();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    layout(request.graph, &request.selected)
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
                notify();
            }
        });
        Self { send, receive }
    }

    pub fn submit(&self, request: Request) -> Result<(), String> {
        self.send
            .send(request)
            .map_err(|_| "레이아웃 작업을 시작하지 못했습니다.".into())
    }

    pub fn try_recv(&self) -> Option<Response> {
        self.receive.try_recv().ok()
    }
}
