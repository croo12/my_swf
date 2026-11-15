use super::super::*;
use super::{fixture, support::Driver};
use crate::file_access::Selection;
use egui_kittest::kittest::Queryable as _;

const SUMMARY: &str = "직접 작성하는 구현 계획";

#[test]
fn headless_typing_commits_one_version_per_idle_edit_and_autosaves_it() {
    let original = fixture::plan();
    let mut driver = Driver::new(original.clone());
    driver.click("계획");
    driver.replace(SUMMARY, "");
    let text = "직접 작성한 계획";
    for ch in text.chars() {
        driver.h.get_by_label(SUMMARY).type_text(&ch.to_string());
        driver.h.step();
        assert_eq!(driver.h.state().session.plan.version, original.version);
    }
    assert_eq!(driver.h.state().session.plan.summary, text);
    driver.idle();
    let first = driver.h.state().session.plan.clone();
    assert_eq!(first.version, original.version + 1);
    assert_eq!(driver.h.state().history, vec![original]);
    assert_eq!(
        factory_session::load(&driver.session_file).unwrap().plan,
        first
    );
    driver.idle();
    assert_eq!(driver.h.state().session.plan, first);

    driver.replace(SUMMARY, "두 번째 편집");
    assert_eq!(driver.h.state().session.plan.version, first.version);
    driver.idle();
    let second = driver.h.state().session.plan.clone();
    assert_eq!(second.version, first.version + 1);
    driver.click("↶");
    assert_eq!(driver.h.state().session.plan, first);
    driver.click("↷");
    assert_eq!(driver.h.state().session.plan, second);
    driver.idle();
    driver.restart();
    assert_eq!(driver.h.state().session.plan, second);
}

#[test]
fn headless_undo_finishes_pending_typing_even_with_unsaved_session_metadata() {
    let original = fixture::plan();
    let mut driver = Driver::new(original.clone());
    driver.h.state_mut().dirty = true;
    driver.click("계획");
    driver.click("features/cart/model 선택");
    driver.replace("이름", "Edited cart");
    driver.replace("책임 · 이 모듈이 하는 일", "Keep totals consistent");
    assert_eq!(driver.h.state().session.plan.version, original.version);
    driver.click("↶");
    assert_eq!(driver.h.state().session.plan, original);
    driver.click("↷");
    let plan = &driver.h.state().session.plan;
    assert_eq!(plan.version, original.version + 1);
    assert_eq!(plan.nodes[0].name, "Edited cart");
    assert_eq!(plan.nodes[0].description, "Keep totals consistent");
}

#[test]
fn headless_exports_and_clipboard_finish_edits_without_creating_extra_versions() {
    let mut driver = Driver::new(fixture::plan());
    driver.click("계획");
    for (selection, label, filename, version) in [
        (Selection::Json, "JSON · 다시 편집할 계획", "plan.json", 2),
        (
            Selection::Markdown,
            "Markdown · AI에 전달할 계획",
            "plan.md",
            3,
        ),
    ] {
        driver.replace(SUMMARY, filename);
        let file = driver.root.path().join(filename);
        driver.dialogs.push(selection, Some(file.clone()));
        driver.click("내보내기");
        driver.click(label);
        let expected = &driver.h.state().session.plan;
        assert_eq!(expected.version, version);
        let text = std::fs::read_to_string(file).unwrap();
        if selection == Selection::Json {
            assert_eq!(Plan::from_json(&text).unwrap(), *expected);
        } else {
            assert_eq!(text, expected.markdown());
        }
        driver.idle();
        assert_eq!(driver.h.state().session.plan.version, version);
    }
    driver.replace(SUMMARY, "클립보드 전달");
    driver.click("내보내기");
    driver.click("Markdown 복사");
    assert_eq!(driver.h.state().session.plan.version, 4);
    assert_eq!(driver.h.state().message, "계획을 클립보드에 복사했습니다.");
    driver.idle();
    assert_eq!(driver.h.state().session.plan.version, 4);
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_save_shortcut_finishes_typing_before_the_idle_deadline() {
    let mut driver = Driver::new(fixture::plan());
    driver.click("계획");
    driver.replace(SUMMARY, "Export immediately");
    let file = driver.root.path().join("shortcut.json");
    driver.dialogs.push(Selection::Json, Some(file.clone()));
    driver
        .h
        .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    driver.pump();
    let exported = Plan::from_json(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(exported.summary, "Export immediately");
    assert_eq!(exported.version, 2);
    driver.idle();
    assert_eq!(driver.h.state().session.plan, exported);
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_exit_commits_pending_typing_but_reverted_text_creates_no_version() {
    let original = fixture::plan();
    let mut driver = Driver::new(original.clone());
    driver.click("계획");
    driver.replace(SUMMARY, "Temporary change");
    driver.replace(SUMMARY, &original.summary);
    driver.idle();
    assert_eq!(driver.h.state().session.plan, original);
    assert!(driver.h.state().history.is_empty());
    driver.replace(SUMMARY, "Saved on exit");
    eframe::App::on_exit(driver.h.state_mut(), None);
    let expected = driver.h.state().session.plan.clone();
    assert_eq!(expected.version, original.version + 1);
    driver.restart();
    assert_eq!(driver.h.state().session.plan, expected);
}

#[test]
fn headless_import_keeps_pending_edit_as_a_separate_undo_step() {
    let original = fixture::plan();
    let mut driver = Driver::new(original.clone());
    driver.click("계획");
    driver.replace(SUMMARY, "Edited before import");
    let mut imported = original.clone();
    imported.version = 25;
    imported.summary = "Imported plan".into();
    let file = driver.root.path().join("import.json");
    std::fs::write(&file, imported.json().unwrap()).unwrap();
    driver.dialogs.push(Selection::Import, Some(file));
    driver.click("JSON 불러오기");
    assert_eq!(driver.h.state().session.plan, imported);
    driver.click("↶");
    assert_eq!(driver.h.state().session.plan.version, original.version + 1);
    assert_eq!(
        driver.h.state().session.plan.summary,
        "Edited before import"
    );
    driver.click("↶");
    assert_eq!(driver.h.state().session.plan, original);
}
