use factory_plan::{Node, Plan};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn plan() -> Plan {
    Plan {
        summary: "Headless UI regression".into(),
        nodes: vec![Node {
            id: "cart".into(),
            name: "Cart".into(),
            path: "features/cart/model".into(),
            files: vec!["cart.ts".into()],
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(super) fn repository(root: &Path) -> PathBuf {
    let repository = root.join("repository");
    for (file, source) in [
        (
            "features/cart/model/index.ts",
            "export { add } from './add';",
        ),
        (
            "features/cart/model/add.ts",
            "import { money } from '@/shared/lib/money'; export function add() { return money(); }",
        ),
        (
            "features/checkout/model/checkout.ts",
            "import { add } from '@/features/cart/model'; import { money } from '@/shared/lib/money'; export function checkout() { return add() + money(); }",
        ),
        (
            "features/unused/model/unused.ts",
            "export function unused() {}",
        ),
        (
            "shared/lib/money.ts",
            "export function money() { return 1; }",
        ),
    ] {
        let path = repository.join("src").join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    repository
}
