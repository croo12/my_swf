use std::{fs, path::Path};

pub const MAX_RUST_LINES: usize = 400;

/// Shared physical-line rule for working-tree builds and staged Git blobs.
pub fn violation(path: &Path, source: &str) -> Option<String> {
    let lines = source.lines().count();
    (lines > MAX_RUST_LINES)
        .then(|| format!("{}: {lines} lines (limit {MAX_RUST_LINES})", path.display()))
}

/// Count physical source lines, including comments and blank lines, in every Rust file.
/// Build output is excluded; nested source directories and unreferenced files are checked.
pub fn check(root: &Path) -> Result<(), String> {
    fn visit(root: &Path, directory: &Path, violations: &mut Vec<String>) -> Result<(), String> {
        let entries = fs::read_dir(directory).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            let path = entry.path();
            if kind.is_dir() {
                if path != root.join("target") {
                    visit(root, &path, violations)?;
                }
            } else if (kind.is_file() || kind.is_symlink())
                && path.extension().is_some_and(|ext| ext == "rs")
            {
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                if let Some(message) = violation(path.strip_prefix(root).unwrap_or(&path), &source)
                {
                    violations.push(message);
                }
            }
        }
        Ok(())
    }
    let mut violations = Vec::new();
    visit(root, root, &mut violations)?;
    violations.sort();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "apps/native Rust files must not exceed {MAX_RUST_LINES} lines. Split responsibilities into modules or shared crates:\n{}",
            violations.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_400_lines_and_rejects_401_in_unreferenced_nested_files() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("future/module");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("unused.rs");
        for newline in ["\n", "\r\n"] {
            fs::write(&file, format!("// source{newline}").repeat(400)).unwrap();
            assert_eq!(check(directory.path()), Ok(()));
            fs::write(&file, format!("// source{newline}").repeat(401)).unwrap();
            let error = check(directory.path()).unwrap_err();
            assert!(error.contains("unused.rs: 401 lines (limit 400)"));
        }
        // A final line without a newline still counts, including a blank/comment-only source.
        fs::write(&file, "\n".repeat(400) + "// last").unwrap();
        assert!(check(directory.path()).is_err());
    }

    #[test]
    fn excludes_build_output_but_checks_build_scripts_and_test_sources() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("target/generated");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("payload.rs"), "// generated\n".repeat(401)).unwrap();
        assert_eq!(check(directory.path()), Ok(()));
        for name in ["build.rs", "tests/large.rs", "src/target/source.rs"] {
            let file = directory.path().join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, "// source\n".repeat(401)).unwrap();
            assert!(check(directory.path()).is_err());
            fs::remove_file(file).unwrap();
        }
    }
}
