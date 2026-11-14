use super::super::{syntax::*, *};

pub(super) fn check(m: &Module, disk: &Disk, author: Author, errors: &mut Vec<String>) {
    let label = if m.name.trim().is_empty() {
        m.id.as_str()
    } else {
        m.name.as_str()
    };
    if m.name.trim().is_empty() {
        errors.push(format!("Module \"{}\" has no name.", m.id));
    }
    // What an unchanged module is for is already in its code; the plan need not restate it.
    if m.responsibility.trim().is_empty() && !(m.kind == Kind::Existing && author == Author::Person)
    {
        errors.push(format!("Module \"{label}\" has no responsibility."));
    }
    if length(&m.name) > MAX_NAME {
        errors.push(format!(
            "Module name \"{label}\" has {} characters; at most {MAX_NAME} are allowed.",
            length(&m.name)
        ));
    }
    if length(&m.responsibility) > MAX_RESPONSIBILITY {
        errors.push(format!("Module \"{label}\": the responsibility has {} characters; at most {MAX_RESPONSIBILITY} are allowed.", length(&m.responsibility)));
    }
    if !is_one_sentence(&m.responsibility) {
        errors.push(format!(
            "Module \"{label}\": the responsibility must be one sentence."
        ));
    }
    // Code a person brought in unchanged is a fact about the repository, not a design to judge.
    let as_found = m.kind == Kind::Existing && author == Author::Person;
    if m.interfaces.len() > MAX_INTERFACES && !as_found {
        errors.push(format!("Module \"{label}\" has {} interfaces; at most {MAX_INTERFACES} are allowed. Split the module.", m.interfaces.len()));
    }
    if m.files.len() > MAX_FILES {
        errors.push(format!(
            "Module \"{label}\" names {} files; at most {MAX_FILES} are allowed. Split the module.",
            m.files.len()
        ));
    }

    // Placement: layer is an enum, so code outside the FSD layers cannot be expressed at all.
    if !is_clean_name(&m.segment) {
        errors.push(format!(
            "Module \"{label}\": segment \"{}\" is not a valid folder name.",
            m.segment
        ));
    }
    match (&m.slice, m.layer.has_slices()) {
        (None, true) => errors.push(format!(
            "Module \"{label}\": layer {} requires a slice.",
            m.layer.name()
        )),
        (Some(_), false) => errors.push(format!(
            "Module \"{label}\": layer {} has no slices; slice must be null.",
            m.layer.name()
        )),
        (Some(slice), true) if !slice.split('/').all(is_clean_name) => errors.push(format!(
            "Module \"{label}\": slice \"{slice}\" is not a valid folder path."
        )),
        _ => {}
    }
    // Files tie the module to the disk: a changed or referenced module must name files that are
    // really at its placement, and a new module must name files that are not there yet.
    if m.files.is_empty() {
        errors.push(format!("Module \"{label}\" names no files."));
    }
    for file in &m.files {
        if !file.split('/').all(is_clean_name) {
            errors.push(format!(
                "Module \"{label}\": file \"{file}\" is not a valid relative file path."
            ));
            continue;
        }
        let extension = file.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        if !SOURCE_EXTENSIONS.contains(&extension) {
            errors.push(format!("Module \"{label}\": file \"{file}\" is not a source file that belongs in the FSD layers."));
            continue;
        }
        let inside = format!("{}/{file}", m.path());
        // A segment can be a single file (types.ts); that file sits next to the segment name.
        let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file);
        let as_segment_file = (stem == m.segment).then(|| {
            format!(
                "{}/{file}",
                m.path()
                    .rsplit_once('/')
                    .map(|(parent, _)| parent)
                    .unwrap_or("")
            )
        });
        let exists = (disk.file_exists)(&inside)
            || as_segment_file
                .as_deref()
                .is_some_and(|path| (disk.file_exists)(path));
        match m.kind {
                Kind::New if exists => errors.push(format!("Module \"{label}\" is marked new but its file {inside} already exists.")),
                Kind::Modify | Kind::Existing if !exists => errors.push(format!("Module \"{label}\" is marked {} but its file {inside} does not exist in the repository.", m.kind.name())),
                _ => {}
            }
    }

    // Interfaces are what make a module reviewable; planned work must state them.
    if m.kind != Kind::Existing && m.interfaces.is_empty() {
        errors.push(format!(
            "Module \"{label}\" ({}) exposes no interface.",
            m.kind.name()
        ));
    }
    let mut interface_names = HashSet::new();
    for interface in &m.interfaces {
        let at = format!("Module \"{label}\": interface \"{}\"", interface.name);
        if interface.name.trim().is_empty() {
            errors.push(format!(
                "Module \"{label}\" has an interface without a name."
            ));
        } else if !interface_names.insert(interface.name.as_str()) {
            errors.push(format!(
                "Module \"{label}\" lists the interface \"{}\" more than once.",
                interface.name
            ));
        }
        if interface.kind == InterfaceKind::Server && !is_server_request(&interface.name) {
            errors.push(format!("Module \"{label}\": server interface \"{}\" must be named METHOD /path, for example POST /api/jobs.", interface.name));
        }

        // Behavior: one condition and one result. Planned work must state the result of
        // everything that does something; a type or a constant does nothing by itself.
        let acts = !matches!(
            interface.kind,
            InterfaceKind::Type | InterfaceKind::Constant
        );
        if m.kind != Kind::Existing && acts && interface.result.trim().is_empty() {
            errors.push(format!("{at} states no result."));
        }
        if length(&interface.condition) > MAX_CONDITION {
            errors.push(format!(
                "{at}: the condition has {} characters; at most {MAX_CONDITION} are allowed.",
                length(&interface.condition)
            ));
        }
        if length(&interface.result) > MAX_RESULT {
            errors.push(format!(
                "{at}: the result has {} characters; at most {MAX_RESULT} are allowed.",
                length(&interface.result)
            ));
        }
        if !is_one_sentence(&interface.condition) || !is_one_sentence(&interface.result) {
            errors.push(format!(
                "{at}: the condition and the result must each be one sentence."
            ));
        }

        // Inputs and outputs are names and types, not descriptions.
        if interface.inputs.len() > MAX_PORTS || interface.outputs.len() > MAX_PORTS {
            errors.push(format!("{at} has more than {MAX_PORTS} inputs or outputs."));
        }
        for input in &interface.inputs {
            if length(input) > MAX_PORT {
                errors.push(format!(
                    "{at}: input \"{input}\" is longer than {MAX_PORT} characters."
                ));
            } else if !is_typed_name(input) {
                errors.push(format!("{at}: input \"{input}\" must be written name: Type with a TypeScript type, for example id: string."));
            }
        }
        for output in &interface.outputs {
            let restated = output.trim() == interface.name.trim()
                || output.trim() == format!("typeof {}", interface.name.trim());
            if restated {
                errors.push(format!("{at}: output \"{output}\" only repeats the name. State the type itself, or leave the outputs empty."));
                continue;
            }
            if length(output) > MAX_PORT {
                errors.push(format!(
                    "{at}: output \"{output}\" is longer than {MAX_PORT} characters."
                ));
            } else if !is_typed_name(output) && !is_type_expression(output) {
                errors.push(format!("{at}: output \"{output}\" must be a TypeScript type or name: Type, for example Promise<boolean>."));
            }
        }
    }
    // A file of types is not a module, with or without default values next to them: it does
    // nothing in the module graph. Its types belong to the module that uses them. A module of
    // constants alone (configuration) is allowed.
    let declares_types = m.interfaces.iter().any(|i| i.kind == InterfaceKind::Type);
    let only_server =
        !m.interfaces.is_empty() && m.interfaces.iter().all(|i| i.kind == InterfaceKind::Server);
    if (declares_types || only_server) && !m.interfaces.iter().any(|i| i.kind.acts()) && !as_found {
        errors.push(format!("\"{label}\" declares types, constants or server requests but no component, hook, function or store, so it is not a module. Put its types and file into the module that uses them."));
    }

    // Claims about the repository are checked against the repository.
    let on_disk = disk.paths.contains(&m.path());
    if m.kind != Kind::New && !on_disk {
        errors.push(format!(
            "Module \"{label}\" is marked {} but {} does not exist in the repository.",
            m.kind.name(),
            m.path()
        ));
    }
}
