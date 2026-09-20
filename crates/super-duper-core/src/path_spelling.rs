//! Equivalent spellings of Windows paths.
//!
//! Scans store canonical Windows paths in verbatim form (`\\?\C:\...`, `\\?\UNC\server\share\...`),
//! while people see, copy, and paste the plain form (`C:\...`, `\\server\share\...`). Lookups that
//! accept a user-supplied path use [`alternate_windows_spelling`] to match either spelling without
//! rewriting what is stored. Display and export surfaces use the one-directional
//! [`to_plain_spelling`] instead, mirroring `DisplayPaths.Plain` in the Windows app. The functions
//! are pure string operations, so they behave the same on every platform.

const VERBATIM_PREFIX: &str = r"\\?\";
const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";

/// Returns the other spelling of a Windows drive or UNC path, or `None` when there is none.
///
/// - `\\?\C:\dir` (or `\\?\C:`) becomes `C:\dir`; `C:\dir` becomes `\\?\C:\dir`.
/// - `\\?\UNC\server\share\dir` becomes `\\server\share\dir`, and the reverse.
///
/// Prefixes are matched case-insensitively. Other verbatim or device forms (`\\?\Volume{...}\`,
/// `\\?\GLOBALROOT\...`, `\\.\...`), drive-relative paths (`C:dir`), relative paths, and POSIX
/// paths have no alternate spelling.
pub fn alternate_windows_spelling(path: &str) -> Option<String> {
    if let Some(rest) = strip_prefix_ignore_ascii_case(path, VERBATIM_UNC_PREFIX) {
        return Some(format!(r"\\{rest}"));
    }
    if let Some(rest) = path.strip_prefix(VERBATIM_PREFIX) {
        return is_drive_root_prefix(rest, true).then(|| rest.to_owned());
    }
    if let Some(rest) = path.strip_prefix(r"\\") {
        // `\\.\` is a device namespace and `\\` alone names no share.
        return (!rest.is_empty() && !rest.starts_with(['?', '.', '\\']))
            .then(|| format!("{VERBATIM_UNC_PREFIX}{rest}"));
    }
    is_drive_root_prefix(path, false).then(|| format!("{VERBATIM_PREFIX}{path}"))
}

/// Returns `path` without a verbatim drive or UNC prefix, for display or export only.
///
/// Unlike [`alternate_windows_spelling`], this never runs the other direction: a path that is
/// already plain, relative, POSIX, or another verbatim form (`\\?\Volume{...}\`,
/// `\\?\GLOBALROOT\...`) is returned unchanged. Never use the result as an identity value —
/// worker requests, review decisions, filter keys, comparisons, and stored records keep the exact
/// original spelling.
pub fn to_plain_spelling(path: &str) -> String {
    if let Some(rest) = strip_prefix_ignore_ascii_case(path, VERBATIM_UNC_PREFIX) {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(VERBATIM_PREFIX)
        && is_drive_root_prefix(rest, true)
    {
        return rest.to_owned();
    }
    path.to_owned()
}

/// `X:` followed by `\` (or by nothing, when `allow_bare_drive` is set).
fn is_drive_root_prefix(value: &str, allow_bare_drive: bool) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && match bytes.get(2) {
            Some(b'\\') => true,
            None => allow_bare_drive,
            Some(_) => false,
        }
}

fn strip_prefix_ignore_ascii_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &value[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_drive_paths_map_to_plain_and_back() {
        assert_eq!(
            alternate_windows_spelling(r"\\?\C:\Users\x\file.bin").as_deref(),
            Some(r"C:\Users\x\file.bin")
        );
        assert_eq!(
            alternate_windows_spelling(r"C:\Users\x\file.bin").as_deref(),
            Some(r"\\?\C:\Users\x\file.bin")
        );
        assert_eq!(alternate_windows_spelling(r"\\?\d:").as_deref(), Some("d:"));
        assert_eq!(
            alternate_windows_spelling(r"d:\").as_deref(),
            Some(r"\\?\d:\")
        );
    }

    #[test]
    fn verbatim_unc_paths_map_to_plain_and_back() {
        assert_eq!(
            alternate_windows_spelling(r"\\?\UNC\server\share\file").as_deref(),
            Some(r"\\server\share\file")
        );
        assert_eq!(
            alternate_windows_spelling(r"\\?\unc\server\share").as_deref(),
            Some(r"\\server\share")
        );
        assert_eq!(
            alternate_windows_spelling(r"\\server\share\file").as_deref(),
            Some(r"\\?\UNC\server\share\file")
        );
    }

    #[test]
    fn other_spellings_have_no_alternate() {
        for path in [
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\Data",
            r"\\?\GLOBALROOT\Device\HarddiskVolume3\Data",
            r"\\?\C:file.bin",
            r"\\?\1:\Data",
            r"\\.\C:\Data",
            r"\\",
            r"\\\server",
            r"C:file.bin",
            "C:",
            r"relative\path",
            "/posix/path",
            "",
        ] {
            assert_eq!(alternate_windows_spelling(path), None, "{path}");
        }
    }

    #[test]
    fn non_ascii_text_is_not_split_inside_a_character() {
        assert_eq!(alternate_windows_spelling("Überraschung"), None);
        assert_eq!(alternate_windows_spelling(r"\\?\Ü"), None);
    }

    #[test]
    fn to_plain_spelling_strips_verbatim_drive_and_unc_prefixes() {
        assert_eq!(
            to_plain_spelling(r"\\?\C:\Users\x\file.bin"),
            r"C:\Users\x\file.bin"
        );
        assert_eq!(to_plain_spelling(r"\\?\d:"), "d:");
        assert_eq!(
            to_plain_spelling(r"\\?\UNC\server\share\file"),
            r"\\server\share\file"
        );
        assert_eq!(
            to_plain_spelling(r"\\?\unc\server\share"),
            r"\\server\share"
        );
    }

    #[test]
    fn to_plain_spelling_never_runs_the_other_direction() {
        for path in [
            r"C:\Users\x\file.bin",
            r"\\server\share\file",
            r"relative\path",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\Data",
            r"\\?\GLOBALROOT\Device\HarddiskVolume3\Data",
            r"\\?\C:file.bin",
            "",
        ] {
            assert_eq!(to_plain_spelling(path), path, "{path}");
        }
    }
}
