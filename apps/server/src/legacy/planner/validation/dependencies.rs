use super::super::{syntax::normalized, *};
use std::collections::HashMap;

pub(super) fn check(
    proposal: &Proposal,
    by_id: &HashMap<&str, &Module>,
    author: Author,
    errors: &mut Vec<String>,
) {
    let modules = &proposal.modules;
    let mut seen = HashSet::new();
    let mut depended_on: HashSet<&str> = HashSet::new();
    // (module id, interface name) pairs that some dependency uses.
    let mut used: HashSet<(&str, &str)> = HashSet::new();
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in &proposal.dependencies {
        let (Some(from), Some(to)) = (by_id.get(d.from.as_str()), by_id.get(d.to.as_str())) else {
            errors.push(format!(
                "Dependency {} -> {} refers to a module that is not in the plan.",
                d.from, d.to
            ));
            continue;
        };
        if d.from == d.to {
            errors.push(format!("Module \"{}\" depends on itself.", from.name));
            continue;
        }
        if !seen.insert((d.from.as_str(), d.to.as_str())) {
            errors.push(format!(
                "Dependency \"{}\" -> \"{}\" is listed more than once.",
                from.name, to.name
            ));
            continue;
        }
        if from.kind == Kind::Existing && author == Author::Agent {
            errors.push(format!(
                "Existing module \"{}\" is unchanged, so the plan must not give it dependencies.",
                from.name
            ));
        }
        // FSD import rule: only downward, or within one slice (app and shared count as one slice each).
        let same_slice = from.layer == to.layer && from.slice == to.slice;
        if !(from.layer.rank() < to.layer.rank() || same_slice) {
            errors.push(format!("\"{}\" ({}) may not import \"{}\" ({}): imports go to a lower layer or stay inside one slice.", from.name, from.path(), to.name, to.path()));
        }
        // A dependency exists for the interfaces it uses, and those must be interfaces the target
        // really offers. Server requests face the server and cannot be used by another module.
        if d.uses.is_empty() {
            errors.push(format!(
                "Dependency \"{}\" -> \"{}\" names no interface it uses.",
                from.name, to.name
            ));
        }
        let mut named = HashSet::new();
        for name in &d.uses {
            if !named.insert(name.as_str()) {
                errors.push(format!(
                    "Dependency \"{}\" -> \"{}\" lists \"{name}\" more than once.",
                    from.name, to.name
                ));
                continue;
            }
            match to.interfaces.iter().find(|i| &i.name == name) {
                None => errors.push(format!("Dependency \"{}\" -> \"{}\" uses \"{name}\", which \"{}\" does not offer.", from.name, to.name, to.name)),
                Some(i) if i.kind == InterfaceKind::Server => errors.push(format!("Dependency \"{}\" -> \"{}\" uses the server request \"{name}\"; a module can only use what another module offers.", from.name, to.name)),
                Some(_) => { used.insert((d.to.as_str(), name.as_str())); }
            }
        }
        depended_on.insert(d.to.as_str());
        graph
            .entry(d.from.as_str())
            .or_default()
            .push(d.to.as_str());
    }
    for m in modules
        .iter()
        .filter(|m| m.kind == Kind::Existing && author == Author::Agent)
    {
        if !depended_on.contains(m.id.as_str()) {
            errors.push(format!(
                "Existing module \"{}\" is in the plan but nothing depends on it.",
                m.name
            ));
        }
    }
    // An existing module is in the plan only for what the planned work uses, so every interface it
    // lists must be used by a dependency.
    for m in modules
        .iter()
        .filter(|m| m.kind == Kind::Existing && author == Author::Agent)
    {
        // Server requests of an existing module are its own business with the server.
        for interface in m
            .interfaces
            .iter()
            .filter(|i| i.kind != InterfaceKind::Server)
        {
            if !used.contains(&(m.id.as_str(), interface.name.as_str())) {
                errors.push(format!("Existing module \"{}\" lists \"{}\", which no dependency uses. List only the interfaces that are used.", m.name, interface.name));
            }
        }
    }
    // An interface belongs to one module; a second module that lists it is re-exporting it.
    let planned: Vec<&Module> = modules
        .iter()
        .filter(|m| m.kind != Kind::Existing)
        .collect();
    let mut owner: HashMap<&str, &Module> = HashMap::new();
    for m in &planned {
        for interface in m
            .interfaces
            .iter()
            .filter(|i| i.kind != InterfaceKind::Server && !i.name.trim().is_empty())
        {
            if let Some(first) = owner.insert(interface.name.as_str(), m) {
                if first.id != m.id {
                    errors.push(format!("Interface \"{}\" is listed by both \"{}\" and \"{}\". It belongs to one module; the other imports it.", interface.name, first.name, m.name));
                }
            }
        }
    }
    // A module whose every acting interface has exactly the signature of an interface of a module
    // it imports from only passes that module through. That is the slice's index, not a module.
    let signature = |i: &Interface| (normalized(&i.inputs), normalized(&i.outputs));
    for m in &planned {
        let own: Vec<&Interface> = m.interfaces.iter().filter(|i| i.kind.acts()).collect();
        let imported: Vec<(&Module, &Interface)> = graph
            .get(m.id.as_str())
            .into_iter()
            .flatten()
            .filter_map(|id| by_id.get(id).copied())
            .flat_map(|dependency| {
                dependency
                    .interfaces
                    .iter()
                    .filter(|i| i.kind.acts())
                    .map(move |i| (dependency, i))
            })
            .collect();
        let twins: Vec<(&Interface, &(&Module, &Interface))> = own
            .iter()
            .filter_map(|i| {
                imported
                    .iter()
                    .find(|(_, other)| {
                        !other.outputs.is_empty() && signature(other) == signature(i)
                    })
                    .map(|twin| (*i, twin))
            })
            .collect();
        if !own.is_empty() && twins.len() == own.len() {
            let (interface, (source, original)) = twins[0];
            errors.push(format!("\"{}\" only passes through \"{}\": {} has the same inputs and outputs as {}. A re-export is the slice's index, not a module.", m.name, source.name, interface.name, original.name));
        }
    }
    // Requests to the server go through the shared API layer: a module that calls the server is
    // either placed in shared/api or reaches a shared/api module through its dependencies.
    let is_api = |id: &str| {
        by_id
            .get(id)
            .is_some_and(|m| m.layer == Layer::Shared && m.segment == API_SEGMENT)
    };
    for m in modules.iter().filter(|m| {
        m.interfaces.iter().any(|i| i.kind == InterfaceKind::Server) && !is_api(m.id.as_str())
    }) {
        if !reaches(m.id.as_str(), &graph, &is_api) {
            errors.push(format!("Module \"{}\" sends requests to the server but does not depend, directly or indirectly, on a module in shared/{API_SEGMENT}.", m.name));
        }
    }
    if let Some(id) = find_cycle(&graph) {
        let name = by_id.get(id).map(|m| m.name.as_str()).unwrap_or(id);
        errors.push(format!("The dependencies form a cycle through \"{name}\"."));
    }
}

/// Whether a module matching `is_target` can be reached from `start` by following dependencies.
fn reaches<'a>(
    start: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    is_target: &dyn Fn(&str) -> bool,
) -> bool {
    let (mut seen, mut stack) = (HashSet::new(), vec![start]);
    while let Some(node) = stack.pop() {
        if !seen.insert(node) {
            continue;
        }
        for &next in graph.get(node).into_iter().flatten() {
            if is_target(next) {
                return true;
            }
            stack.push(next);
        }
    }
    false
}

/// Returns a module id that lies on a dependency cycle, if there is one.
fn find_cycle<'a>(graph: &HashMap<&'a str, Vec<&'a str>>) -> Option<&'a str> {
    fn visit<'a>(
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        done: &mut HashSet<&'a str>,
        path: &mut HashSet<&'a str>,
    ) -> Option<&'a str> {
        if done.contains(node) {
            return None;
        }
        if !path.insert(node) {
            return Some(node);
        }
        for &next in graph.get(node).into_iter().flatten() {
            if let Some(found) = visit(next, graph, done, path) {
                return Some(found);
            }
        }
        path.remove(node);
        done.insert(node);
        None
    }
    let (mut done, mut path) = (HashSet::new(), HashSet::new());
    graph
        .keys()
        .find_map(|&node| visit(node, graph, &mut done, &mut path))
}
