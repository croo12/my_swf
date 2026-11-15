use super::super::*;
use super::{fixture, support::Driver};
use crate::file_access::Selection;
use egui_kittest::kittest::Queryable as _;

#[test]
fn headless_layers_and_selection_use_real_scanning_and_native_routing() {
    let mut driver = Driver::new(Plan::default());
    let repository = fixture::repository(driver.root.path());
    driver.dialogs.push(Selection::Repository, Some(repository));
    driver.click("폴더 열기");
    assert_eq!(
        driver
            .h
            .state()
            .repository
            .as_ref()
            .unwrap()
            .tree
            .file_count,
        5
    );
    assert!(
        driver
            .scene()
            .edges
            .iter()
            .all(|route| route.points.is_empty())
    );
    driver.click("features 레이어 펼치기");
    assert!(driver.h.state().inspected_path.is_empty());
    assert!(
        driver
            .scene()
            .edges
            .iter()
            .all(|route| route.points.is_empty())
    );
    driver.click("features/cart 선택");
    assert_eq!(driver.h.state().inspected_path, "features/cart");
    assert_eq!(
        driver
            .scene()
            .edges
            .iter()
            .filter(|route| !route.points.is_empty())
            .count(),
        2
    );
    let item = |path: &str| {
        driver
            .scene()
            .boxes
            .iter()
            .find(|item| item.path == path)
            .unwrap()
    };
    assert!(item("features/unused").dimmed);
    assert!(!item("features/checkout").dimmed);
    assert!(item("features/unused").rect.left() > item("features/checkout").rect.right());
    assert_eq!(
        item("features/cart").rect.top(),
        item("features/checkout").rect.top()
    );
    driver.click("features 레이어 접기");
    assert!(driver.h.state().inspected_path.is_empty());
    assert!(
        driver
            .scene()
            .edges
            .iter()
            .all(|route| route.points.is_empty())
    );
}

#[test]
fn headless_search_reveals_index_imports_and_copies_a_real_group_into_the_plan() {
    let mut driver = Driver::new(Plan::default());
    let repository = fixture::repository(driver.root.path());
    driver.dialogs.push(Selection::Repository, Some(repository));
    driver.click("폴더 열기");
    driver.replace("저장소 검색", "features/cart/model");
    driver.click("features/cart/model");
    driver.click("공개 진입점: features/cart/model/index.ts");
    assert_eq!(
        driver.h.state().inspected_path,
        "features/cart/model/index.ts"
    );
    assert!(
        driver
            .h
            .state()
            .repository_graph
            .expanded
            .contains("features/cart/model")
    );
    let routed: Vec<_> = driver
        .scene()
        .edges
        .iter()
        .filter(|route| !route.points.is_empty())
        .collect();
    assert!(
        routed
            .iter()
            .any(|route| route.edge.from == "features/cart/model/index.ts")
    );
    assert!(
        routed
            .iter()
            .any(|route| route.edge.to == "features/cart/model/index.ts")
    );
    driver.replace("저장소 검색", "features/cart/model");
    driver.click("features/cart/model");
    driver.click("이 그룹을 계획에 추가");
    assert!(driver.h.state().planning);
    let plan = &driver.h.state().session.plan;
    assert_eq!(plan.nodes.len(), 1);
    assert_eq!(plan.nodes[0].path, "features/cart/model");
    assert_eq!(plan.nodes[0].files, ["add.ts", "index.ts"]);
    assert!(plan.errors().is_empty());
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_ctrl_wheel_zooms_only_the_graph_and_normal_wheel_keeps_its_scale() {
    let mut driver = Driver::new(Plan::default());
    let repository = fixture::repository(driver.root.path());
    driver.dialogs.push(Selection::Repository, Some(repository));
    driver.click("폴더 열기");
    driver.click("features 레이어 펼치기");
    driver.click("features/cart 선택");
    let before = driver.h.state().repository_graph.zoom;
    let pointer = driver.h.get_by_label("features/cart 선택").rect().center();
    let wheel = |modifiers| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -2.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers,
    };
    driver.h.hover_at(pointer);
    driver
        .h
        .event_modifiers(wheel(egui::Modifiers::CTRL), egui::Modifiers::CTRL);
    driver.pump();
    assert!(driver.h.state().repository_graph.zoom < before);
    let after = driver.h.state().repository_graph.zoom;
    driver.h.event(wheel(egui::Modifiers::NONE));
    driver.pump();
    assert_eq!(driver.h.state().repository_graph.zoom, after);
    let inspector = driver.h.get_by_label("이 그룹을 계획에 추가");
    driver.h.hover_at(inspector.rect().center());
    driver
        .h
        .event_modifiers(wheel(egui::Modifiers::CTRL), egui::Modifiers::CTRL);
    driver.pump();
    assert_eq!(driver.h.state().repository_graph.zoom, after);
}

#[test]
fn headless_slice_import_includes_all_files_supports_undo_and_rejects_duplicates() {
    let mut driver = Driver::new(Plan::default());
    let repository = fixture::repository(driver.root.path());
    std::fs::write(
        repository.join("src/features/cart/index.ts"),
        "export { add } from './model';",
    )
    .unwrap();
    let ui = repository.join("src/features/cart/ui");
    std::fs::create_dir_all(&ui).unwrap();
    std::fs::write(ui.join("cart.ts"), "export const Cart = () => 1;").unwrap();
    driver.dialogs.push(Selection::Repository, Some(repository));
    driver.click("폴더 열기");
    driver.click("features 레이어 펼치기");
    driver.click("features/cart 선택");
    driver.click("이 그룹을 계획에 추가");
    assert!(driver.h.state().planning);
    let imported = driver.h.state().session.plan.clone();
    assert_eq!(imported.nodes.len(), 1);
    assert_eq!(imported.nodes[0].path, "features/cart");
    let mut files = imported.nodes[0].files.clone();
    files.sort();
    assert_eq!(
        files,
        ["index.ts", "model/add.ts", "model/index.ts", "ui/cart.ts"]
    );
    assert!(imported.errors().is_empty());
    let json = driver.root.path().join("slice-plan.json");
    driver.dialogs.push(Selection::Json, Some(json.clone()));
    driver.click("내보내기");
    driver.click("JSON · 다시 편집할 계획");
    assert_eq!(
        Plan::from_json(&std::fs::read_to_string(json).unwrap()).unwrap(),
        imported
    );
    driver.click("↶");
    assert!(driver.h.state().session.plan.nodes.is_empty());
    driver.click("↷");
    assert_eq!(driver.h.state().session.plan, imported);
    driver.click("저장소");
    driver.click("이 그룹을 계획에 추가");
    assert_eq!(driver.h.state().session.plan, imported);
    assert_eq!(driver.h.state().message, "이 그룹은 이미 계획에 있습니다.");
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_routing_failure_keeps_cards_clickable_and_recovers_on_another_selection() {
    let mut driver = Driver::new(Plan::default());
    let repository = fixture::repository(driver.root.path());
    driver.dialogs.push(Selection::Repository, Some(repository));
    driver.click("폴더 열기");
    driver.click("features 레이어 펼치기");
    driver.click("features/cart 선택");
    let snapshot = driver.h.state().repository.clone().unwrap();
    driver
        .h
        .state_mut()
        .repository_graph
        .test_routing_failure(&snapshot, "features/cart");
    driver.h.step();
    driver
        .h
        .get_by_label("연결선을 계산하지 못했습니다. 카드 탐색은 계속 사용할 수 있습니다.");
    driver.h.get_by_label("features/checkout 선택").click();
    driver.h.step();
    assert_eq!(driver.h.state().inspected_path, "features/checkout");
    driver.pump();
    assert!(
        driver
            .scene()
            .edges
            .iter()
            .any(|route| !route.points.is_empty())
    );
    assert!(driver.dialogs.exhausted());
}
