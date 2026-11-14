use super::*;

#[test]
fn an_interface_needs_a_unique_name_but_not_inputs_or_outputs() {
    let mut bare = interface("Thing", "component");
    bare["inputs"] = json!([]);
    bare["outputs"] = json!([]);
    assert_eq!(check_interface(bare), NONE);
    let mut bad = module("b", "new", "features", Some("x"), "ui");
    bad["interfaces"] = json!([
        interface("", "hook"),
        interface("A", "hook"),
        interface("A", "hook")
    ]);
    let errors = check(&proposal(vec![bad], vec![]), &[]);
    assert!(
        has(&errors, "without a name") && has(&errors, "more than once"),
        "{errors:?}"
    );
}

#[test]
fn behavior_is_a_condition_and_a_result() {
    // Something that acts must state its result; the condition may be empty.
    let mut always = interface("format", "function");
    always["condition"] = json!("");
    assert_eq!(check_interface(always), NONE);
    let mut silent = interface("cancelJob", "function");
    silent["result"] = json!(" ");
    assert!(has(&check_interface(silent), "states no result"));
    // A type states no behavior, but a file of types is still not a module.
    let mut shape = interface("Job", "type");
    shape["condition"] = json!("");
    shape["result"] = json!("");
    let mut m = module("a", "new", "features", Some("x"), "model");
    m["interfaces"] = json!([shape, interface("cancelJob", "function")]);
    assert_eq!(check(&proposal(vec![m], vec![]), &[]), NONE);
    // Existing modules are only referenced, so their behavior may be left out.
    let mut used = interface("apiRequest", "function");
    used["result"] = json!("");
    let mut api = module("api", "existing", "shared", None, "api");
    api["interfaces"] = json!([used]);
    let p = proposal(
        vec![module("a", "new", "features", Some("x"), "ui"), api],
        vec![("a", "api")],
    );
    assert_eq!(check(&p, &["shared/api"]), NONE);
}

#[test]
fn prose_is_limited_in_length_and_to_one_sentence() {
    let mut m = module("a", "new", "features", Some("x"), "ui");
    m["name"] = json!("n".repeat(MAX_NAME + 1));
    m["responsibility"] = json!("Does one thing. And then another thing.");
    let mut long = interface("cancelJob", "function");
    long["condition"] = json!("w".repeat(MAX_CONDITION + 1));
    long["result"] = json!("가".repeat(MAX_RESULT + 1));
    m["interfaces"] = json!([long]);
    let mut p = proposal(vec![m], vec![]);
    p.summary = "s".repeat(MAX_SUMMARY + 1);
    let errors = check(&p, &[]);
    for part in [
        "Module name",
        "must be one sentence",
        "condition has",
        "result has",
        "The summary has",
    ] {
        assert!(has(&errors, part), "missing {part:?} in {errors:?}");
    }
    // Limits count characters, not bytes: 100 Hangul syllables fit.
    let mut korean = interface("cancelJob", "function");
    korean["result"] = json!("가".repeat(MAX_RESULT));
    assert_eq!(check_interface(korean), NONE);
}

#[test]
fn lists_are_limited_in_size() {
    let mut m = module("a", "new", "features", Some("x"), "ui");
    m["interfaces"] = Value::Array(
        (0..=MAX_INTERFACES)
            .map(|i| interface(&format!("f{i}"), "function"))
            .collect(),
    );
    m["files"] = Value::Array((0..=MAX_FILES).map(|i| json!(format!("f{i}.ts"))).collect());
    let errors = check(&proposal(vec![m], vec![]), &[]);
    assert!(
        has(&errors, "interfaces; at most") && has(&errors, "files; at most"),
        "{errors:?}"
    );
    let mut wide = interface("f", "function");
    wide["inputs"] = Value::Array(
        (0..=MAX_PORTS)
            .map(|i| json!(format!("a{i}: string")))
            .collect(),
    );
    assert!(has(&check_interface(wide), "inputs or outputs"));
}

#[test]
fn inputs_and_outputs_are_names_and_types_not_descriptions() {
    let with = |inputs: Value, outputs: Value| {
        let mut i = interface("f", "function");
        i["inputs"] = inputs;
        i["outputs"] = outputs;
        check_interface(i)
    };
    // Accepted: typed names, optional names, and the usual type syntax.
    assert_eq!(
        with(
            json!([
                "id: string",
                "target?: string | null",
                "options: { retry: boolean; times: number }",
                "onDone: (job: Job) => void"
            ]),
            json!([
                "Promise<boolean>",
                "job: JobView | null",
                "Record<string, readonly Job[]>",
                "keyof typeof STATUS",
                "'queued' | 'running'",
                "T extends Job ? string : never"
            ])
        ),
        NONE
    );
    // Rejected: descriptions, missing types, prose in any language, unbalanced brackets.
    assert!(has(
        &with(json!(["the job id"]), json!([])),
        "must be written name: Type"
    ));
    assert!(has(
        &with(json!(["id"]), json!([])),
        "must be written name: Type"
    ));
    assert!(has(
        &with(json!(["id: 작업 ID 경로 매개변수"]), json!([])),
        "must be written name: Type"
    ));
    assert!(has(
        &with(json!(["id: the id of the job"]), json!([])),
        "must be written name: Type"
    ));
    assert!(has(
        &with(
            json!([]),
            json!(["작업 표시 및 취소 제어에 필요한 상태 구조"])
        ),
        "must be a TypeScript type"
    ));
    assert!(has(
        &with(json!([]), json!(["the current job state"])),
        "must be a TypeScript type"
    ));
    assert!(has(
        &with(json!([]), json!(["Promise<boolean"])),
        "must be a TypeScript type"
    ));
}

#[test]
fn a_file_of_types_is_not_a_module() {
    let mut types = module("contract", "new", "features", Some("x"), "model");
    types["interfaces"] = json!([
        interface("RunContract", "type"),
        interface("RunResult", "type")
    ]);
    assert!(has(
        &check(&proposal(vec![types], vec![]), &[]),
        "so it is not a module"
    ));
    // Types next to something usable are fine: they are part of that module's interface.
    let mut with_types = module("model", "new", "features", Some("x"), "model");
    with_types["interfaces"] = json!([
        interface("RunContract", "type"),
        interface("runSentence", "function")
    ]);
    assert_eq!(check(&proposal(vec![with_types], vec![]), &[]), NONE);
}

#[test]
fn types_with_a_default_value_are_still_not_a_module() {
    // The shape that slipped through: three types and one constant holding an empty value.
    let mut contract = module("contract", "new", "features", Some("x"), "model");
    let mut empty = interface("EMPTY_RESULT", "constant");
    empty["condition"] = json!("");
    empty["result"] = json!("");
    contract["interfaces"] = json!([
        empty.clone(),
        interface("RunInput", "type"),
        interface("RunResult", "type")
    ]);
    assert!(has(
        &check(&proposal(vec![contract], vec![]), &[]),
        "so it is not a module"
    ));
    // Constants alone are configuration, which is a module.
    let mut config = module("config", "new", "shared", None, "config");
    config["interfaces"] = json!([empty]);
    assert_eq!(check(&proposal(vec![config], vec![]), &[]), NONE);
}

#[test]
fn an_output_that_repeats_the_name_is_rejected() {
    let mut shape = interface("RunInput", "type");
    shape["outputs"] = json!(["RunInput"]);
    let mut m = module("a", "new", "features", Some("x"), "model");
    m["interfaces"] = json!([shape, interface("run", "function")]);
    assert!(has(
        &check(&proposal(vec![m], vec![]), &[]),
        "only repeats the name"
    ));
    let mut value = interface("Sentence", "constant");
    value["outputs"] = json!(["typeof Sentence"]);
    assert!(has(&check_interface(value), "only repeats the name"));
}
