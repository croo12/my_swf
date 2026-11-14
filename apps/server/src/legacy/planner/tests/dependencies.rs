use super::*;

#[test]
fn rejects_imports_that_go_upward_or_across_slices() {
    let p = proposal(
        vec![
            module("entity", "new", "entities", Some("job"), "model"),
            module("feature", "new", "features", Some("cancel"), "ui"),
            module("other", "new", "features", Some("other"), "ui"),
        ],
        vec![("entity", "feature"), ("feature", "other")],
    );
    let errors = check(&p, &[]);
    assert_eq!(
        errors
            .iter()
            .filter(|e| e.contains("may not import"))
            .count(),
        2
    );
}

#[test]
fn rejects_missing_interfaces_and_dangling_dependencies() {
    let mut m = module("a", "new", "features", Some("x"), "ui");
    m["interfaces"] = json!([]);
    let errors = check(&proposal(vec![m], vec![("a", "nowhere")]), &[]);
    assert!(
        has(&errors, "exposes no interface") && has(&errors, "not in the plan"),
        "{errors:?}"
    );
}

#[test]
fn an_interface_has_one_owner_and_a_pass_through_is_not_a_module() {
    // "api" re-exports the hook of "hook" under another name, and lists its type again.
    let mut hook = module("hook", "new", "features", Some("x"), "lib");
    hook["interfaces"] = json!([
        interface("useRuntime", "hook"),
        interface("RunResult", "type")
    ]);
    let mut api = module("api", "new", "features", Some("x"), "public-api");
    let mut reexport = interface("useSentences", "hook");
    reexport["outputs"] = interface("useRuntime", "hook")["outputs"].clone();
    api["interfaces"] = json!([reexport, interface("RunResult", "type")]);
    let errors = check(
        &proposal(vec![hook.clone(), api], vec![("api", "hook")]),
        &[],
    );
    assert!(has(&errors, "is listed by both"), "{errors:?}");
    assert!(has(&errors, "only passes through"), "{errors:?}");
    // A module that adds something of its own next to a wrapper is a module.
    let mut different = interface("useSentences", "hook");
    different["outputs"] = json!(["SlotContent"]);
    let mut widget = module("widget", "new", "widgets", Some("editor"), "model");
    widget["interfaces"] = json!([different]);
    assert_eq!(
        check(&proposal(vec![hook, widget], vec![("widget", "hook")]), &[]),
        NONE
    );
}

#[test]
fn server_requests_are_interfaces_and_pass_through_shared_api() {
    let request = interface("POST /api/jobs/{id}/cancel", "server");
    let mut model = module("model", "new", "features", Some("cancel"), "model");
    model["interfaces"] = json!([interface("cancelJob", "function"), request]);
    let api = module("api", "existing", "shared", None, "api");
    let lib = module("lib", "existing", "shared", None, "lib");
    // Directly through shared/api, or indirectly through another module: accepted.
    assert_eq!(
        check(
            &proposal(vec![model.clone(), api.clone()], vec![("model", "api")]),
            &["shared/api"]
        ),
        NONE
    );
    let mut ui = module("ui", "new", "features", Some("cancel"), "ui");
    ui["interfaces"] = json!([
        interface("CancelButton", "component"),
        interface("GET /api/jobs/{id}", "server")
    ]);
    let plain_model = module("model", "new", "features", Some("cancel"), "model");
    assert_eq!(
        check(
            &proposal(
                vec![ui, plain_model, api.clone()],
                vec![("ui", "model"), ("model", "api")]
            ),
            &["shared/api"]
        ),
        NONE
    );
    // Bypassing shared/api: rejected.
    let errors = check(
        &proposal(vec![model.clone(), lib], vec![("model", "lib")]),
        &["shared/lib"],
    );
    assert!(
        has(
            &errors,
            "does not depend, directly or indirectly, on a module in shared/api"
        ),
        "{errors:?}"
    );
    // A server interface that is not named as a request: rejected.
    let mut misnamed = model;
    misnamed["interfaces"] = json!([
        interface("cancelJob", "function"),
        interface("cancel the job", "server")
    ]);
    assert!(has(
        &check(
            &proposal(vec![misnamed, api], vec![("model", "api")]),
            &["shared/api"]
        ),
        "must be named METHOD /path"
    ));
}

#[test]
fn a_dependency_names_the_interfaces_it_uses() {
    let mut api = module("api", "existing", "shared", None, "api");
    api["interfaces"] = json!([
        interface("apiRequest", "function"),
        interface("errorMessage", "function"),
        interface("GET /api/status", "server")
    ]);
    let model = module("model", "new", "features", Some("x"), "model");
    let with = |uses: Value| {
        let mut p: Value = json!({"summary":"s","modules":[model.clone(), api.clone()],"dependencies":[{"from":"model","to":"api","uses":uses}]});
        // errorMessage is only listed when the case under test uses it.
        if !p["dependencies"][0]["uses"]
            .as_array()
            .is_some_and(|u| u.contains(&json!("errorMessage")))
        {
            p["modules"][1]["interfaces"] = json!([
                interface("apiRequest", "function"),
                interface("GET /api/status", "server")
            ]);
        }
        check(
            &serde_json::from_value(p).expect("fixture parses"),
            &["shared/api"],
        )
    };
    assert_eq!(with(json!(["apiRequest", "errorMessage"])), NONE);
    assert!(has(&with(json!([])), "names no interface it uses"));
    assert!(has(
        &with(json!(["apiRequest", "apiRequest"])),
        "more than once"
    ));
    assert!(has(
        &with(json!(["apiRequest", "fetchEverything"])),
        "does not offer"
    ));
    assert!(has(
        &with(json!(["apiRequest", "GET /api/status"])),
        "uses the server request"
    ));
    // An existing module may list only what is used.
    let p = json!({"summary":"s","modules":[model, module("api", "existing", "shared", None, "api")],"dependencies":[{"from":"model","to":"api","uses":["run_api"]}]});
    let mut extra = p.clone();
    extra["modules"][1]["interfaces"] = json!([
        interface("run_api", "function"),
        interface("unusedHelper", "function")
    ]);
    assert_eq!(
        check(
            &serde_json::from_value(p).expect("fixture parses"),
            &["shared/api"]
        ),
        NONE
    );
    assert!(has(
        &check(
            &serde_json::from_value(extra).expect("fixture parses"),
            &["shared/api"]
        ),
        "which no dependency uses"
    ));
}

#[test]
fn rejects_unused_existing_modules_and_cycles() {
    let p = proposal(
        vec![
            module("a", "new", "features", Some("x"), "ui"),
            module("b", "new", "features", Some("x"), "model"),
            module("lib", "existing", "shared", None, "lib"),
        ],
        vec![("a", "b"), ("b", "a")],
    );
    let errors = check(&p, &["shared/lib"]);
    assert!(has(&errors, "nothing depends on it") && has(&errors, "form a cycle"));
}
