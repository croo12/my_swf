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
