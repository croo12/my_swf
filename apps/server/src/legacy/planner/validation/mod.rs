mod dependencies;
mod module;
use super::{syntax::length, *};
use std::collections::HashMap;

pub fn validate(proposal: &Proposal, disk: &Disk, author: Author) -> Vec<String> {
    let mut errors = vec![];
    let modules = &proposal.modules;
    if modules.is_empty() {
        errors.push("The plan has no modules.".to_owned());
    }
    if modules.len() > MAX_MODULES {
        errors.push(format!(
            "The plan has {} modules; at most {MAX_MODULES} are allowed.",
            modules.len()
        ));
    }
    if !modules.iter().any(|m| m.kind != Kind::Existing) {
        errors.push(
            "The plan creates or changes nothing: every module is marked existing.".to_owned(),
        );
    }
    if length(&proposal.summary) > MAX_SUMMARY {
        errors.push(format!(
            "The summary has {} characters; at most {MAX_SUMMARY} are allowed.",
            length(&proposal.summary)
        ));
    }

    let mut by_id: HashMap<&str, &Module> = HashMap::new();
    for m in modules {
        let label = if m.name.trim().is_empty() {
            m.id.as_str()
        } else {
            m.name.as_str()
        };
        if m.id.trim().is_empty() {
            errors.push(format!("Module \"{label}\" has no id."));
        } else if by_id.insert(m.id.as_str(), m).is_some() {
            errors.push(format!("Module id \"{}\" is used more than once.", m.id));
        }
        module::check(m, disk, author, &mut errors);
    }
    dependencies::check(proposal, &by_id, author, &mut errors);
    errors
}
