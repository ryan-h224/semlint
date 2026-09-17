use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub message: String,
}

/// Lints one version string per line. Blank lines and lines starting with
/// '#' are treated as comments and skipped, so a versions file can carry
/// its own notes.
pub fn lint_str(input: &str, lenient: bool) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, raw_line) in input.lines().enumerate() {
        let line = idx + 1;
        let trimmed_start = raw_line.trim_start();
        let indent = raw_line.len() - trimmed_start.len();
        let token = trimmed_start.trim_end();
        if token.is_empty() || token.starts_with('#') {
            continue;
        }
        check_version(token, line, indent + 1, lenient, &mut findings);
    }
    findings
}

fn check_version(token: &str, line: usize, column: usize, lenient: bool, findings: &mut Vec<Finding>) {
    let mut rest = token;

    // A leading 'v' is the single most common real-world deviation (git
    // tags, GitHub releases). Strict mode flags it; lenient strips it and
    // keeps checking the rest of the string.
    if let Some(stripped) = rest.strip_prefix('v').or_else(|| rest.strip_prefix('V')) {
        if lenient {
            rest = stripped;
        } else {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!(
                    "'{}' has a 'v' prefix; strict SemVer core starts with a digit (pass --lenient to allow it)",
                    token
                ),
            });
            rest = stripped;
        }
    }

    let (core_and_pre, build) = match rest.split_once('+') {
        Some((l, r)) => (l, Some(r)),
        None => (rest, None),
    };
    let (core, pre) = match core_and_pre.split_once('-') {
        Some((l, r)) => (l, Some(r)),
        None => (core_and_pre, None),
    };

    let parts: Vec<&str> = if core.is_empty() {
        Vec::new()
    } else {
        core.split('.').collect()
    };

    if parts.len() != 3 {
        let looks_numeric = !parts.is_empty()
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
        if lenient && parts.len() < 3 && looks_numeric {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Warning,
                message: format!(
                    "'{}' has {} of 3 numeric components; missing components are treated as zero (--lenient)",
                    token,
                    parts.len()
                ),
            });
        } else {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!(
                    "version core must be exactly major.minor.patch (three dot-separated numbers), found '{}'",
                    core
                ),
            });
        }
    }

    for (i, part) in parts.iter().take(3).enumerate() {
        let name = ["major", "minor", "patch"][i];
        if part.is_empty() {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!("{} component is empty in '{}'", name, token),
            });
        } else if !part.chars().all(|c| c.is_ascii_digit()) {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!("{} component '{}' is not a plain number", name, part),
            });
        } else if part.len() > 1 && part.starts_with('0') {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!("{} component '{}' has a leading zero, which SemVer forbids", name, part),
            });
        }
    }

    if let Some(pre) = pre {
        check_dotted_identifiers(pre, "pre-release", true, token, line, column, findings);
    }

    if let Some(build) = build {
        check_dotted_identifiers(build, "build metadata", false, token, line, column, findings);
    }
}

/// Checks the dot-separated identifiers that make up a pre-release or
/// build-metadata section. Leading zeros are only forbidden for numeric
/// pre-release identifiers -- build metadata has no such rule in the spec.
fn check_dotted_identifiers(
    section: &str,
    label: &str,
    forbid_leading_zero_on_numeric: bool,
    token: &str,
    line: usize,
    column: usize,
    findings: &mut Vec<Finding>,
) {
    if section.is_empty() {
        findings.push(Finding {
            line,
            column,
            severity: Severity::Error,
            message: format!("{} in '{}' is empty", label, token),
        });
        return;
    }

    for ident in section.split('.') {
        if ident.is_empty() {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!("{} in '{}' has an empty identifier between dots", label, token),
            });
            continue;
        }
        if !ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!(
                    "{} identifier '{}' in '{}' contains characters outside [0-9A-Za-z-]",
                    label, ident, token
                ),
            });
            continue;
        }
        let is_numeric = ident.chars().all(|c| c.is_ascii_digit());
        if forbid_leading_zero_on_numeric && is_numeric && ident.len() > 1 && ident.starts_with('0') {
            findings.push(Finding {
                line,
                column,
                severity: Severity::Error,
                message: format!("numeric {} identifier '{}' in '{}' has a leading zero", label, ident, token),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errors(input: &str, lenient: bool) -> Vec<Finding> {
        lint_str(input, lenient)
            .into_iter()
            .filter(|f| f.severity == Severity::Error)
            .collect()
    }

    fn warnings(input: &str, lenient: bool) -> Vec<Finding> {
        lint_str(input, lenient)
            .into_iter()
            .filter(|f| f.severity == Severity::Warning)
            .collect()
    }

    #[test]
    fn valid_version_has_no_findings() {
        assert!(lint_str("1.2.3", false).is_empty());
        assert!(lint_str("1.2.3-alpha.1+build.5", false).is_empty());
        assert!(lint_str("0.0.0", false).is_empty());
    }

    #[test]
    fn blank_and_comment_lines_are_skipped() {
        assert!(lint_str("\n# not a version\n   \n", false).is_empty());
    }

    #[test]
    fn v_prefix_is_an_error_in_strict_mode() {
        let findings = errors("v1.2.3", false);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("'v' prefix"));
    }

    #[test]
    fn v_prefix_is_stripped_and_ignored_in_lenient_mode() {
        assert!(lint_str("v1.2.3", true).is_empty());
        assert!(lint_str("V1.2.3", true).is_empty());
    }

    #[test]
    fn short_form_is_an_error_in_strict_mode() {
        let findings = errors("1.2", false);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("major.minor.patch"));
    }

    #[test]
    fn short_form_is_a_warning_in_lenient_mode() {
        let w = warnings("1.2", true);
        assert_eq!(w.len(), 1);
        assert!(w[0].message.contains("2 of 3"));
        assert!(errors("1.2", true).is_empty());
    }

    #[test]
    fn non_numeric_short_form_is_still_an_error_in_lenient_mode() {
        let findings = errors("1.x", true);
        assert!(findings.iter().any(|f| f.message.contains("major.minor.patch")));
        assert!(findings
            .iter()
            .any(|f| f.message.contains("minor component 'x' is not a plain number")));
    }

    #[test]
    fn too_many_core_components_is_an_error() {
        let findings = errors("1.2.3.4", false);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("major.minor.patch"));
    }

    #[test]
    fn empty_core_component_is_an_error() {
        let findings = errors("1..3", false);
        assert!(findings.iter().any(|f| f.message.contains("minor component is empty")));
    }

    #[test]
    fn non_numeric_core_component_is_an_error() {
        let findings = errors("1.a.3", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("minor component 'a' is not a plain number")));
    }

    #[test]
    fn leading_zero_core_component_is_an_error() {
        let findings = errors("1.02.3", false);
        assert!(findings.iter().any(|f| f.message.contains("leading zero")));
    }

    #[test]
    fn single_digit_zero_component_is_not_a_leading_zero_error() {
        assert!(lint_str("0.1.0", false).is_empty());
    }

    #[test]
    fn empty_pre_release_section_is_an_error() {
        let findings = errors("1.2.3-", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("pre-release in '1.2.3-' is empty")));
    }

    #[test]
    fn empty_pre_release_identifier_between_dots_is_an_error() {
        let findings = errors("1.2.3-alpha..1", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("empty identifier between dots")));
    }

    #[test]
    fn invalid_pre_release_characters_are_an_error() {
        let findings = errors("1.2.3-alpha_beta", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("outside [0-9A-Za-z-]")));
    }

    #[test]
    fn leading_zero_numeric_pre_release_identifier_is_an_error() {
        let findings = errors("1.2.3-01", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("numeric pre-release identifier '01'")));
    }

    #[test]
    fn leading_zero_alphanumeric_pre_release_identifier_is_allowed() {
        assert!(lint_str("1.2.3-0alpha", false).is_empty());
    }

    #[test]
    fn empty_build_metadata_section_is_an_error() {
        let findings = errors("1.2.3+", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("build metadata in '1.2.3+' is empty")));
    }

    #[test]
    fn empty_build_metadata_identifier_between_dots_is_an_error() {
        let findings = errors("1.2.3+build..5", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("build metadata") && f.message.contains("empty identifier between dots")));
    }

    #[test]
    fn invalid_build_metadata_characters_are_an_error() {
        let findings = errors("1.2.3+build_5", false);
        assert!(findings
            .iter()
            .any(|f| f.message.contains("build metadata identifier") && f.message.contains("outside [0-9A-Za-z-]")));
    }

    #[test]
    fn leading_zero_build_metadata_identifier_is_allowed() {
        assert!(lint_str("1.2.3+001", false).is_empty());
    }

    #[test]
    fn line_and_column_are_reported_for_multiple_lines() {
        let findings = errors("1.2.3\n  v2.0.0\n", false);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[0].column, 3);
    }

    #[test]
    fn multiple_findings_on_one_token_are_all_reported() {
        let findings = errors("v1.02.3-01", false);
        assert!(findings.len() >= 3);
    }
}
