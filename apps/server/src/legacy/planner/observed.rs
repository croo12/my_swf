use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;

/// One import that exists in the repository.
#[derive(Debug, Clone)]
pub struct ObservedImport {
    /// Placement of the importing file, and the file itself relative to the source root.
    pub from: String,
    pub file: String,
    /// Placement that provides the name, and the file that declares it (empty when unknown).
    pub to: String,
    pub provider: String,
    /// The imported name; "*" where the import does not name what it takes.
    pub name: String,
}

/// Reads the imports out of a repository scan.
pub fn observed_imports(scan: &Value) -> Vec<ObservedImport> {
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    // Imports inside one placement connect files of the same segment; the plan's surroundings
    // are about what lies outside a placement.
    scan.get("records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|r| ObservedImport {
            from: text(r, "from"),
            file: text(r, "file"),
            to: text(r, "to"),
            provider: text(r, "provider"),
            name: text(r, "name"),
        })
        .filter(|import| import.from != import.to)
        .collect()
}

const UNNAMED_IMPORT: &str = "*";

fn without_extension(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((stem, extension)) if SOURCE_EXTENSIONS.contains(&extension) => stem,
        _ => path,
    }
}

/// Whether `file` (relative to the source root, possibly without an extension, possibly a folder)
/// is one of the module's files.
fn owns(module: &Module, file: &str) -> bool {
    if file.is_empty() {
        return false;
    }
    let file = without_extension(file);
    let path = module.path();
    module.files.iter().any(|own| {
        let inside = format!("{path}/{}", without_extension(own));
        // A segment can be a single file that sits next to the segment name.
        let beside = (without_extension(own) == module.segment).then(|| path.clone());
        [Some(inside), beside].into_iter().flatten().any(|own| {
            own == file || own == format!("{file}/index") || own.starts_with(&format!("{file}/"))
        })
    })
}

/// The code around the plan: files the plan does not mention that import from a planned module, or
/// that a changed module imports from. These come from the repository's imports, not from the
/// proposal, so they show what the planned change is really connected to.
///
/// An import belongs to a planned module if the module lists the imported name as an interface or
/// owns the file that declares it; an importing file belongs to the module that lists it. Imports
/// that touch a planned placement but none of the planned modules' files are left out.
pub fn surroundings(proposal: &Proposal, observed: &[ObservedImport]) -> (Vec<Value>, Vec<Value>) {
    let lists = |m: &Module, name: &str| {
        name != UNNAMED_IMPORT && m.interfaces.iter().any(|i| i.name == name)
    };
    // A name may move between segments of one slice; app and shared have no slices.
    let same_slice = |m: &Module, path: &str| {
        m.path() == path
            || m.slice.as_ref().is_some_and(|slice| {
                m.layer.has_slices() && path.starts_with(&format!("{}/{slice}/", m.layer.name()))
            })
    };
    let context_id = |path: &str| format!("observed-{}", path.replace('/', "--"));

    let mut context: Vec<String> = vec![];
    let mut order: Vec<(String, String)> = vec![];
    let mut uses: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut link = |from: String, to: String, name: &str, context_path: Option<&str>| {
        if let Some(new) = context_path.filter(|new| !context.iter().any(|path| path == new)) {
            context.push(new.to_owned());
        }
        let key = (from, to);
        if !uses.contains_key(&key) {
            order.push(key.clone());
        }
        let list = uses.entry(key).or_default();
        if name != UNNAMED_IMPORT && !list.iter().any(|listed| listed == name) {
            list.push(name.to_owned());
        }
    };

    for import in observed {
        let importer = proposal
            .modules
            .iter()
            .find(|m| m.path() == import.from && owns(m, &import.file));
        let planned_provider = proposal
            .modules
            .iter()
            .filter(|m| m.kind != Kind::Existing)
            .find(|m| same_slice(m, &import.to) && lists(m, &import.name))
            .or_else(|| {
                proposal.modules.iter().find(|m| {
                    m.kind == Kind::Modify && m.path() == import.to && owns(m, &import.provider)
                })
            });
        match (importer, planned_provider) {
            // Unplanned code that uses a planned module.
            (None, Some(provider)) => link(
                context_id(&import.from),
                provider.id.clone(),
                &import.name,
                Some(&import.from),
            ),
            // Unplanned code that a changed module uses. What the plan already lists is not repeated.
            (Some(m), None) if m.kind == Kind::Modify => {
                let listed = proposal.modules.iter().any(|other| {
                    other.path() == import.to
                        && (lists(other, &import.name) || owns(other, &import.provider))
                });
                if listed {
                    continue;
                }
                // If the plan has exactly one module at that placement, the import is of that
                // module: show it there instead of a second card for the same place, unless the
                // plan already states a dependency between the two.
                let there: Vec<&Module> = proposal
                    .modules
                    .iter()
                    .filter(|other| other.path() == import.to)
                    .collect();
                match there.as_slice() {
                    [only] => {
                        if !proposal
                            .dependencies
                            .iter()
                            .any(|d| d.from == m.id && d.to == only.id)
                        {
                            link(m.id.clone(), only.id.clone(), &import.name, None);
                        }
                    }
                    _ => link(
                        m.id.clone(),
                        context_id(&import.to),
                        &import.name,
                        Some(&import.to),
                    ),
                }
            }
            _ => {}
        }
    }

    let nodes = context.iter().map(|path| json!({
        "id":context_id(path),"kind":Kind::Existing.name(),"path":path,"observed":true,
        "description":"계획에 없는 기존 코드. 저장소의 import로 확인한 연결입니다.","interfaces":[],"files":[]
    })).collect();
    let edges = order.iter().enumerate().map(|(i, key)| json!({
        "id":format!("observed-{}",i + 1),"from":key.0,"to":key.1,"type":"imports","uses":uses[key],"observed":true
    })).collect();
    (nodes, edges)
}
