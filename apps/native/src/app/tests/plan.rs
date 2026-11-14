use super::super::*;
use super::{fixture, support::Driver};

#[test]
fn headless_add_edit_undo_and_redo_use_the_real_workbench() {
    let mut driver = Driver::new(Plan::default());
    driver.click("계획");
    driver.click("＋ 첫 모듈 추가");
    assert_eq!(driver.h.state().session.plan.nodes.len(), 1);
    driver.replace("이름", "Edited module");
    assert_eq!(driver.h.state().session.plan.nodes[0].name, "Edited module");
    let edited = driver.h.state().session.plan.clone();
    driver.click("↶");
    assert_ne!(driver.h.state().session.plan, edited);
    driver.click("↷");
    assert_eq!(driver.h.state().session.plan, edited);
    assert!(driver.dialogs.exhausted());
}

#[test]
fn headless_existing_plan_cards_can_be_selected_and_edited() {
    let mut driver = Driver::new(fixture::plan());
    driver.click("계획");
    driver.click("features/cart/model 선택");
    assert_eq!(driver.h.state().selected.as_deref(), Some("cart"));
    driver.replace("책임 · 이 모듈이 하는 일", "Keep cart totals consistent");
    assert_eq!(
        driver.h.state().session.plan.nodes[0].description,
        "Keep cart totals consistent"
    );
}

#[test]
fn headless_interfaces_and_dependencies_can_be_added_changed_and_removed() {
    let mut plan = fixture::plan();
    plan.nodes.push(factory_core::plan::Node {
        id: "checkout".into(),
        name: "Checkout".into(),
        path: "features/checkout/model".into(),
        ..Default::default()
    });
    let mut driver = Driver::new(plan);
    driver.click("계획");
    driver.click("features/cart/model 선택");
    driver.click("의존 대상");
    driver.click("Checkout · features/checkout/model");
    driver.click("이 모듈 → 대상 모듈 연결");
    let edges = &driver.h.state().session.plan.edges;
    assert_eq!(edges.len(), 1);
    assert_eq!((&*edges[0].from, &*edges[0].to), ("cart", "checkout"));
    driver.click("관계 종류");
    driver.click("imports");
    assert_eq!(driver.h.state().session.plan.edges[0].kind, "imports");
    driver.click("연결 해제");
    assert!(driver.h.state().session.plan.edges.is_empty());
    driver.click("＋ 추가");
    assert_eq!(driver.h.state().session.plan.nodes[0].interfaces.len(), 1);
    driver.click("인터페이스 제거");
    assert!(driver.h.state().session.plan.nodes[0].interfaces.is_empty());
    driver.click("모듈 제거");
    assert_eq!(driver.h.state().session.plan.nodes.len(), 1);
    assert_eq!(driver.h.state().session.plan.nodes[0].id, "checkout");
    assert!(driver.h.state().selected.is_none());
}
