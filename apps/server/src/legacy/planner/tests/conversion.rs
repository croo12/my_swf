use super::*;

#[test]
fn plan_shows_the_unplanned_code_around_changed_modules() {
    let p = proposal(
        vec![module(
            "filter",
            "modify",
            "features",
            Some("variant-filter"),
            "model",
        )],
        vec![],
    );
    let observed = vec![
        // Uses an interface the changed module lists.
        import(
            "widgets/table/ui",
            "widgets/table/ui/Table.tsx",
            "features/variant-filter/model",
            "features/variant-filter/model/other.ts",
            "run_filter",
        ),
        // Uses something declared in the changed module's file.
        import(
            "pages/sample/ui",
            "pages/sample/ui/Page.tsx",
            "features/variant-filter/model",
            "features/variant-filter/model/a.ts",
            "FilterState",
        ),
        // Uses another file of the same segment, which the plan does not touch.
        import(
            "pages/other/ui",
            "pages/other/ui/Other.tsx",
            "features/variant-filter/model",
            "features/variant-filter/model/unrelated.ts",
            "unrelated",
        ),
        // The changed module's file imports unplanned code; another file of the segment does too.
        import(
            "features/variant-filter/model",
            "features/variant-filter/model/a.ts",
            "shared/lib",
            "shared/lib/date.ts",
            "formatDate",
        ),
        import(
            "features/variant-filter/model",
            "features/variant-filter/model/unrelated.ts",
            "shared/ui",
            "shared/ui/Button.tsx",
            "Button",
        ),
    ];
    let plan = to_plan(&p, &observed);
    let ids = |key: &str| -> Vec<String> {
        plan[key]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["observed"] == true)
            .map(|v| v["id"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        ids("nodes"),
        [
            "observed-widgets--table--ui",
            "observed-pages--sample--ui",
            "observed-shared--lib"
        ]
    );
    let edges: Vec<(String, String, String)> = plan["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["from"].as_str().unwrap().to_owned(),
                e["to"].as_str().unwrap().to_owned(),
                e["uses"][0].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        edges,
        [
            (
                "observed-widgets--table--ui".to_owned(),
                "filter".to_owned(),
                "run_filter".to_owned()
            ),
            (
                "observed-pages--sample--ui".to_owned(),
                "filter".to_owned(),
                "FilterState".to_owned()
            ),
            (
                "filter".to_owned(),
                "observed-shared--lib".to_owned(),
                "formatDate".to_owned()
            ),
        ]
    );
}

#[test]
fn the_schema_cannot_express_code_outside_the_fsd_layers() {
    let outside = json!({"summary":"s","dependencies":[],"modules":[module("srv", "new", "server", None, "src")]});
    assert!(serde_json::from_value::<Proposal>(outside).is_err());
}

#[test]
fn rejects_unknown_fields_and_unknown_kinds() {
    let mut m = module("a", "new", "features", Some("x"), "ui");
    m["behaviors"] = json!(["free prose"]);
    assert!(serde_json::from_value::<Proposal>(
        json!({"summary":"s","dependencies":[],"modules":[m]})
    )
    .is_err());
    let mut k = module("a", "new", "features", Some("x"), "ui");
    k["interfaces"] = json!([interface("x", "service")]);
    assert!(serde_json::from_value::<Proposal>(
        json!({"summary":"s","dependencies":[],"modules":[k]})
    )
    .is_err());
}
