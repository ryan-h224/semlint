//! Pulls the package version out of a Cargo.toml or package.json without a
//! real TOML/JSON parser. Only the one field semlint cares about is found,
//! and its position is kept so findings point at the right line and column.

use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct Located {
    pub line: usize,
    pub column: usize,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    CargoToml,
    PackageJson,
}

/// Decides by file name only, so `semlint path/to/Cargo.toml` works and a
/// plain versions file called anything else stays in line-per-version mode.
pub fn detect(path: &str) -> Option<Kind> {
    match Path::new(path).file_name()?.to_str()? {
        "Cargo.toml" => Some(Kind::CargoToml),
        "package.json" => Some(Kind::PackageJson),
        _ => None,
    }
}

pub fn find_version(kind: Kind, content: &str) -> Option<Located> {
    match kind {
        Kind::CargoToml => find_cargo_version(content),
        Kind::PackageJson => find_package_json_version(content),
    }
}

/// Looks for `version = "..."` inside [package] or [workspace.package].
/// `version.workspace = true` has no string to check and is skipped.
fn find_cargo_version(content: &str) -> Option<Located> {
    let mut in_package = false;
    for (idx, raw) in content.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with('[') {
            let header = trimmed.split('#').next().unwrap_or("").trim();
            in_package = header == "[package]" || header == "[workspace.package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let rest = match trimmed.strip_prefix("version") {
            Some(r) => r.trim_start(),
            None => continue,
        };
        let rest = match rest.strip_prefix('=') {
            Some(r) => r.trim_start(),
            None => continue,
        };
        let quote = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => q,
            _ => continue,
        };
        let body = &rest[1..];
        let end = body.find(quote)?;
        let column = raw.len() - rest.len() + 2;
        return Some(Located {
            line: idx + 1,
            column,
            value: body[..end].to_string(),
        });
    }
    None
}

/// Walks the text tracking brace depth and string state, and returns the
/// string value of a top-level "version" key. Nested "version" keys (inside
/// dependencies, engines, etc.) are ignored. Only a value on the same line
/// as its key is handled, which is how npm writes the file.
fn find_package_json_version(content: &str) -> Option<Located> {
    let b = content.as_bytes();
    let mut i = 0;
    let mut line = 1;
    let mut line_start = 0;
    let mut depth = 0usize;

    while i < b.len() {
        match b[i] {
            b'\n' => {
                line += 1;
                line_start = i + 1;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.saturating_sub(1),
            b'"' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                if j >= b.len() {
                    return None;
                }
                if depth == 1 && &content[start..j] == "version" {
                    if let Some(found) = string_value_after_key(content, j + 1, line, line_start) {
                        return Some(found);
                    }
                }
                i = j;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn string_value_after_key(content: &str, from: usize, line: usize, line_start: usize) -> Option<Located> {
    let b = content.as_bytes();
    let mut k = from;
    let skip_blanks = |k: &mut usize| {
        while *k < b.len() && (b[*k] == b' ' || b[*k] == b'\t') {
            *k += 1;
        }
    };
    skip_blanks(&mut k);
    if b.get(k) != Some(&b':') {
        return None;
    }
    k += 1;
    skip_blanks(&mut k);
    if b.get(k) != Some(&b'"') {
        return None;
    }
    let start = k + 1;
    let mut end = start;
    while end < b.len() && b[end] != b'"' && b[end] != b'\n' {
        end += if b[end] == b'\\' { 2 } else { 1 };
    }
    if end >= b.len() || b[end] != b'"' {
        return None;
    }
    Some(Located {
        line,
        column: start - line_start + 1,
        value: content[start..end].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_matches_on_file_name() {
        assert_eq!(detect("Cargo.toml"), Some(Kind::CargoToml));
        assert_eq!(detect("crates/x/Cargo.toml"), Some(Kind::CargoToml));
        assert_eq!(detect("web/package.json"), Some(Kind::PackageJson));
        assert_eq!(detect("versions.txt"), None);
        assert_eq!(detect("-"), None);
    }

    #[test]
    fn cargo_version_in_package_section() {
        let toml = "[package]\nname = \"x\"\nversion = \"1.2.3\"\n";
        let found = find_version(Kind::CargoToml, toml).unwrap();
        assert_eq!(found.line, 3);
        assert_eq!(found.column, 12);
        assert_eq!(found.value, "1.2.3");
    }

    #[test]
    fn cargo_dependency_versions_are_ignored() {
        let toml = "[dependencies]\nversion = \"9.9.9\"\n\n[package]\nversion = '0.1'\n";
        let found = find_version(Kind::CargoToml, toml).unwrap();
        assert_eq!(found.line, 5);
        assert_eq!(found.value, "0.1");
    }

    #[test]
    fn cargo_workspace_package_version_is_found() {
        let toml = "[workspace.package]\nversion = \"2.0.0\" # shared\n";
        let found = find_version(Kind::CargoToml, toml).unwrap();
        assert_eq!(found.value, "2.0.0");
    }

    #[test]
    fn cargo_inherited_version_yields_nothing() {
        let toml = "[package]\nname = \"x\"\nversion.workspace = true\n";
        assert_eq!(find_version(Kind::CargoToml, toml), None);
    }

    #[test]
    fn cargo_key_with_similar_prefix_is_not_a_version() {
        let toml = "[package]\nversioning = \"1.0.0\"\n";
        assert_eq!(find_version(Kind::CargoToml, toml), None);
    }

    #[test]
    fn package_json_top_level_version() {
        let json = "{\n  \"name\": \"x\",\n  \"version\": \"1.2.3\"\n}\n";
        let found = find_version(Kind::PackageJson, json).unwrap();
        assert_eq!(found.line, 3);
        assert_eq!(found.column, 15);
        assert_eq!(found.value, "1.2.3");
    }

    #[test]
    fn package_json_nested_version_is_ignored() {
        let json = "{\n  \"dependencies\": { \"version\": \"7.0.0\" },\n  \"version\": \"1.0.0\"\n}\n";
        let found = find_version(Kind::PackageJson, json).unwrap();
        assert_eq!(found.line, 3);
        assert_eq!(found.value, "1.0.0");
    }

    #[test]
    fn package_json_string_value_named_version_is_not_a_key() {
        let json = "{\"name\": \"version\", \"version\": \"3.1.4\"}";
        let found = find_version(Kind::PackageJson, json).unwrap();
        assert_eq!(found.value, "3.1.4");
    }

    #[test]
    fn package_json_without_version_yields_nothing() {
        assert_eq!(find_version(Kind::PackageJson, "{\"name\": \"x\"}"), None);
        assert_eq!(find_version(Kind::PackageJson, "{\"version\": 3}"), None);
    }
}
