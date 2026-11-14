use super::*;

#[test]
fn accepts_a_well_formed_plan() {
    let p = proposal(
        vec![
            module("ui", "new", "features", Some("cancel-job"), "ui"),
            module("model", "modify", "features", Some("plan-job"), "model"),
            module("api", "existing", "shared", None, "api"),
        ],
        vec![("ui", "api"), ("model", "api")],
    );
    assert_eq!(check(&p, &["features/plan-job/model", "shared/api"]), NONE);
}

#[test]
fn rejects_claims_about_modules_that_are_not_on_disk() {
    let p = proposal(
        vec![module("m", "modify", "features", Some("ghost"), "model")],
        vec![],
    );
    assert!(has(&check(&p, &[]), "does not exist in the repository"));
}

#[test]
fn rejects_files_that_contradict_the_disk_or_do_not_belong_in_fsd() {
    let mut no_files = module("a", "modify", "features", Some("x"), "ui");
    no_files["files"] = json!([]);
    let mut wrong_file = module("b", "modify", "features", Some("x"), "ui");
    wrong_file["files"] = json!(["missing.ts"]);
    let mut rust = module("c", "new", "app", None, "server");
    rust["files"] = json!(["main.rs"]);
    let already_there = module("d", "new", "features", Some("x"), "ui");
    let errors = check(
        &proposal(vec![no_files, wrong_file, rust, already_there], vec![]),
        &["features/x/ui"],
    );
    assert!(has(&errors, "names no files"), "{errors:?}");
    assert!(has(&errors, "missing.ts does not exist"), "{errors:?}");
    assert!(
        has(&errors, "not a source file that belongs in the FSD layers"),
        "{errors:?}"
    );
    assert!(has(&errors, "already exists"), "{errors:?}");
}

#[test]
fn rejects_slice_rules_violations() {
    let p = proposal(
        vec![
            module("a", "new", "features", None, "ui"),
            module("b", "new", "shared", Some("x"), "ui"),
            module("c", "new", "pages", Some("../etc"), "ui"),
        ],
        vec![],
    );
    let errors = check(&p, &[]);
    assert!(
        has(&errors, "requires a slice")
            && has(&errors, "slice must be null")
            && has(&errors, "not a valid folder path")
    );
}
