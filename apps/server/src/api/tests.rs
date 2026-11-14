use super::*;
use axum::http::header::AUTHORIZATION;
use std::collections::HashMap;
use tokio::sync::Mutex;

#[tokio::test]
async fn hierarchy_requires_auth_and_reads_nested_groups_without_writing() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("src/features/cart/model/actions");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(fixture.path().join("src/shared/lib")).unwrap();
    let index = fixture.path().join("src/features/cart/index.ts");
    std::fs::write(&index, "export { add } from './model/actions';").unwrap();
    std::fs::write(source.join("index.ts"), "export const add = () => 1;").unwrap();
    std::fs::write(
        fixture.path().join("src/shared/lib/index.ts"),
        "export const value = 1;",
    )
    .unwrap();
    let state = Arc::new(AppState {
        repository: fixture.path().into(),
        data_dir: fixture.path().join("unused-data"),
        herdr_bin: String::new(),
        api_token: "test-fixture-token-not-a-credential".into(),
        guard: Arc::new(Mutex::new(())),
        jobs: Arc::new(Mutex::new(HashMap::new())),
    });
    let denied = repository_tree(State(state.clone()), HeaderMap::new())
        .await
        .unwrap_err();
    assert_eq!(denied.0, StatusCode::UNAUTHORIZED);
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        "Bearer test-fixture-token-not-a-credential"
            .parse()
            .unwrap(),
    );
    let Json(response) = repository_tree(State(state), headers).await.unwrap();
    let snapshot: repository::Snapshot = serde_json::from_value(response).unwrap();
    assert_eq!(snapshot.source_root, "src");
    assert_eq!(
        snapshot
            .tree
            .find("features/cart/model/actions")
            .unwrap()
            .kind,
        "group"
    );
    assert!(!fixture.path().join("unused-data").exists());
    assert_eq!(
        std::fs::read_to_string(index).unwrap(),
        "export { add } from './model/actions';"
    );
}

#[tokio::test]
async fn remote_client_reads_the_real_http_route_and_reports_bad_credentials() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("src/features/cart/model");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("index.ts"), "export const add = () => 1;").unwrap();
    let shared = fixture.path().join("src/shared/lib");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("index.ts"), "export const value = 1;").unwrap();
    let state = Arc::new(AppState {
        repository: fixture.path().into(),
        data_dir: fixture.path().join("unused-data"),
        herdr_bin: String::new(),
        api_token: "test-fixture-token-not-a-credential".into(),
        guard: Arc::new(Mutex::new(())),
        jobs: Arc::new(Mutex::new(HashMap::new())),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let (snapshot, denied) = tokio::task::spawn_blocking(move || {
        (
            factory_remote_repository::fetch(&base, "test-fixture-token-not-a-credential"),
            factory_remote_repository::fetch(&base, "wrong-fixture-token-not-a-credential"),
        )
    })
    .await
    .unwrap();
    server.abort();
    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot.source_root, "src");
    assert_eq!(snapshot.tree.file_count, 2);
    assert!(snapshot.tree.find("features/cart/model/index.ts").is_some());
    assert!(denied.err().unwrap().contains("401"));
    assert!(!fixture.path().join("unused-data").exists());
}
