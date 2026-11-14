use super::*;

#[test]
fn standalone_comparison_exports_metrics_and_svg_without_a_ui_runtime() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("repository/src");
    fs::create_dir_all(source.join("features/cart/model")).unwrap();
    fs::create_dir_all(source.join("shared/lib")).unwrap();
    fs::write(
        source.join("features/cart/model/cart.ts"),
        "import { money } from '../../../shared/lib/money'; export const total = money;",
    )
    .unwrap();
    fs::write(
        source.join("shared/lib/money.ts"),
        "export const money = 1;",
    )
    .unwrap();
    let output = fixture.path().join("comparison");
    run(&fixture.path().join("repository"), &output, |_| [1, 2, 3]).unwrap();
    let report: Value =
        serde_json::from_slice(&fs::read(output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["files"], 2);
    let scenarios = report["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 3);
    for scenario in scenarios {
        let name = scenario["scenario"].as_str().unwrap();
        assert_eq!(scenario["edges"], 1);
        for engine in ["elk", "previous"] {
            assert_eq!(scenario[engine]["routed_edges"], 1);
            assert_eq!(scenario[engine]["node_overlaps"], 0);
            assert_eq!(scenario[engine]["containment_errors"], 0);
            let svg = fs::read_to_string(output.join(format!("{name}-{engine}.svg"))).unwrap();
            assert!(svg.contains("stroke=\"#010203\""));
            assert!(svg.contains("<polyline"));
        }
        for suffix in ["input", "elk"] {
            let json = fs::read(output.join(format!("{name}-{suffix}.json"))).unwrap();
            serde_json::from_slice::<Value>(&json).unwrap();
        }
    }
}
