use super::*;
use std::fs;

fn repository_fixture() -> tempfile::TempDir {
    let fixture = tempfile::tempdir().unwrap();
    for (file, text) in [
        (
            "src/features/cart/index.ts",
            "export { add } from './model/actions';",
        ),
        (
            "src/features/cart/model/actions/index.ts",
            "export { add } from './add';",
        ),
        (
            "src/features/cart/model/actions/add.ts",
            "export const add = () => 1;",
        ),
        (
            "src/features/checkout/model/checkout.ts",
            "import { add } from '@/features/cart'; export const checkout = add;",
        ),
        ("src/shared/lib/money.ts", "export const money = 1;"),
    ] {
        let path = fixture.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fixture
}

#[test]
fn imports_a_nested_group_with_its_real_files_and_public_interfaces() {
    let fixture = repository_fixture();
    let snapshot = factory_repository::analyze(fixture.path()).unwrap();
    let entry = snapshot.tree.find("features/cart/model/actions").unwrap();
    let node = Node::from_repository(&snapshot, entry);
    assert_eq!(node.path, "features/cart/model");
    assert_eq!(node.kind, "modify");
    assert_eq!(node.files, ["actions/add.ts", "actions/index.ts"]);
    assert_eq!(node.interfaces.len(), 1);
    assert_eq!(node.interfaces[0].name, "add");
    assert_eq!(node.interfaces[0].kind, "function");
    assert!(node.interfaces[0].result.is_empty());
    assert!(
        crate::Plan {
            nodes: vec![node],
            ..Default::default()
        }
        .errors()
        .is_empty()
    );
}

#[test]
fn imports_the_whole_slice_with_its_public_index_and_nested_files() {
    let fixture = repository_fixture();
    let snapshot = factory_repository::analyze(fixture.path()).unwrap();
    let entry = snapshot.tree.find("features/cart").unwrap();
    assert_eq!(entry.kind, "slice");
    let node = Node::from_repository(&snapshot, entry);
    assert_eq!(node.path, "features/cart");
    assert_eq!(node.name, "cart");
    assert_eq!(node.kind, "modify");
    let mut files = node.files.clone();
    files.sort();
    assert_eq!(
        files,
        ["index.ts", "model/actions/add.ts", "model/actions/index.ts"]
    );
    assert_eq!(node.interfaces.len(), 1);
    assert_eq!(node.interfaces[0].name, "add");
    let plan = crate::Plan {
        nodes: vec![node],
        ..Default::default()
    };
    assert_eq!(crate::Plan::from_json(&plan.json().unwrap()).unwrap(), plan);
    let markdown = plan.markdown();
    assert!(markdown.contains("`features/cart/index.ts`"));
    assert!(markdown.contains("`features/cart/model/actions/add.ts`"));
}
