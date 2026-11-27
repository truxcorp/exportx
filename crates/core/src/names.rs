//! File and folder names that work on macOS, Windows and Linux.

use std::fmt;

/// Why a name can't be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    /// One of `\ / : * ? " < > |` or a control character.
    ForbiddenChar(char),
    /// CON, PRN, AUX, NUL, COM1-9, LPT1-9, with or without an extension.
    ReservedOnWindows,
    /// Windows drops a trailing dot or space.
    TrailingDotOrSpace,
    /// Hidden on macOS and Linux.
    LeadingDot,
}

impl fmt::Display for NameProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::i18n::Lang::En.name_problem(*self))
    }
}

const FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
const RESERVED: &[&str] = &["CON", "PRN", "AUX", "NUL"];

/// Check one file or folder name (a single path segment).
pub fn check_name(name: &str) -> Result<(), NameProblem> {
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    if let Some(c) = name
        .chars()
        .find(|c| FORBIDDEN.contains(c) || c.is_control())
    {
        return Err(NameProblem::ForbiddenChar(c));
    }
    if name.starts_with('.') {
        return Err(NameProblem::LeadingDot);
    }
    if name.ends_with(['.', ' ']) {
        return Err(NameProblem::TrailingDotOrSpace);
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let numbered = |p: &str| {
        stem.strip_prefix(p)
            .is_some_and(|d| d.len() == 1 && matches!(d.as_bytes()[0], b'1'..=b'9'))
    };
    if RESERVED.contains(&stem.as_str()) || numbered("COM") || numbered("LPT") {
        return Err(NameProblem::ReservedOnWindows);
    }
    Ok(())
}
