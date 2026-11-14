use super::super::*;
use super::{fixture, support::Driver};
use crate::file_access::Selection;

#[test]
fn headless_json_and_markdown_exports_use_scripted_dialogs() {
    let mut driver = Driver::new(fixture::plan());
    let json = driver.root.path().join("plan.json");
    let markdown = driver.root.path().join("plan.md");
    driver.dialogs.push(Selection::Json, Some(json.clone()));
    driver.click("내보내기");
    driver.click("JSON · 다시 편집할 계획");
    let exported = Plan::from_json(&std::fs::read_to_string(json).unwrap()).unwrap();
    assert_eq!(exported, driver.h.state().session.plan);
    driver
        .dialogs
        .push(Selection::Markdown, Some(markdown.clone()));
    driver.click("내보내기");
    driver.click("Markdown · AI에 전달할 계획");
    assert_eq!(
        std::fs::read_to_string(markdown).unwrap(),
        exported.markdown()
    );
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_import_rejects_bad_data_preserves_the_plan_and_supports_cancellation() {
    let mut driver = Driver::new(Plan::default());
    let valid = driver.root.path().join("valid.json");
    let invalid = driver.root.path().join("invalid.json");
    let expected = fixture::plan();
    std::fs::write(&valid, expected.json().unwrap()).unwrap();
    std::fs::write(&invalid, r#"{"unexpected":true}"#).unwrap();
    driver.dialogs.push(Selection::Import, Some(valid));
    driver.click("JSON 불러오기");
    assert!(driver.h.state().planning);
    assert_eq!(driver.h.state().session.plan, expected);
    driver.dialogs.push(Selection::Import, Some(invalid));
    driver.click("JSON 불러오기");
    assert!(driver.h.state().error);
    assert!(driver.h.state().message.contains("nodes와 edges"));
    assert_eq!(driver.h.state().session.plan, expected);
    driver.dialogs.push(Selection::Import, None);
    driver.click("JSON 불러오기");
    assert!(!driver.h.state().busy);
    assert!(!driver.h.state().error);
    assert_eq!(driver.h.state().session.plan, expected);
    assert_eq!(driver.h.state().message, "취소했습니다.");
    driver.click("↶");
    assert!(driver.h.state().session.plan.nodes.is_empty());
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_autosave_and_restart_use_only_the_injected_profile() {
    let mut driver = Driver::new(fixture::plan());
    driver.click("계획");
    driver.click("features/cart/model 선택");
    driver.replace("이름", "Persisted cart");
    let deadline = Instant::now() + Duration::from_secs(5);
    while crate::storage::load(&driver.session_file)
        .unwrap()
        .plan
        .nodes[0]
        .name
        != "Persisted cart"
    {
        assert!(Instant::now() < deadline, "Autosave did not finish");
        driver.h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    let expected = driver.h.state().session.plan.clone();
    driver.restart();
    assert_eq!(driver.h.state().session.plan, expected);
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_corrupt_saved_draft_is_preserved_on_exit() {
    let root = tempfile::tempdir().unwrap();
    let session = root.path().join("profile/session.json");
    std::fs::create_dir_all(session.parent().unwrap()).unwrap();
    let original = b"{ invalid saved draft";
    std::fs::write(&session, original).unwrap();
    let mut driver = Driver::from_root(root);
    assert!(!driver.h.state().storage_enabled);
    assert!(driver.h.state().error);
    driver.click("계획");
    driver.click("＋ 첫 모듈 추가");
    eframe::App::on_exit(driver.h.state_mut(), None);
    assert_eq!(std::fs::read(session).unwrap(), original);
}
