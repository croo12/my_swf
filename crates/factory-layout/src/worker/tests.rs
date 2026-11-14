use super::*;

fn request(id: u64, selected: &str) -> Request {
    Request {
        id,
        selected: selected.into(),
        graph: Graph::default(),
    }
}

#[test]
fn requests_queued_during_layout_coalesce_to_the_latest_selection() {
    let (started, start) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (wake, notified) = mpsc::channel();
    let timeout = Duration::from_secs(5);
    let worker = Worker::spawn(
        move || {
            wake.send(()).unwrap();
        },
        move |_, selected| {
            if selected == "first" {
                started.send(()).unwrap();
                wait.recv_timeout(timeout).unwrap();
            }
            Err(selected.to_owned())
        },
    );
    worker.submit(request(1, "first")).unwrap();
    start.recv_timeout(timeout).unwrap();
    worker.submit(request(2, "stale")).unwrap();
    worker.submit(request(3, "latest")).unwrap();
    release.send(()).unwrap();
    for (id, selection) in [(1, "first"), (3, "latest")] {
        notified.recv_timeout(timeout).unwrap();
        let response = worker.receive.recv_timeout(timeout).unwrap();
        assert_eq!(response.id, id);
        assert_eq!(response.result.err().unwrap(), selection);
    }
    assert!(worker.try_recv().is_none());
}

#[test]
fn a_failed_calculation_does_not_stop_the_worker() {
    let worker = Worker::spawn(
        || {},
        |_, selected| {
            if selected == "panic" {
                panic!("test calculation failure");
            }
            Scene::focused(Graph::default(), "")
        },
    );
    let timeout = Duration::from_secs(5);
    worker.submit(request(1, "panic")).unwrap();
    let failure = worker.receive.recv_timeout(timeout).unwrap();
    assert_eq!(failure.id, 1);
    assert!(
        failure
            .result
            .err()
            .unwrap()
            .contains("레이아웃 계산 중 오류")
    );
    worker.submit(request(2, "recovered")).unwrap();
    let response = worker.receive.recv_timeout(timeout).unwrap();
    assert_eq!(response.id, 2);
    assert!(response.result.is_ok());
}
