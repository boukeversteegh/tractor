//! Git integration for filtering files by change status.
//!
//! Resolves changed files by invoking `git diff --name-only` with a
//! user-provided diff specification (e.g. `HEAD~3`, `main..HEAD`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Run `git diff --name-only` with the given spec and return the set of
/// changed file paths, resolved relative to `cwd`.
///
/// The spec is split on whitespace and passed as separate arguments to git,
/// so `"main..HEAD"` becomes `["main..HEAD"]` and `"HEAD -- src/"` becomes
/// `["HEAD", "--", "src/"]`.
///
/// Deleted files are excluded via `--diff-filter=ACMR` (Added, Copied,
/// Modified, Renamed).
pub fn git_changed_files(
    spec: &str,
    cwd: &Path,
) -> Result<HashSet<PathBuf>, Box<dyn std::error::Error>> {
    let args: Vec<&str> = spec.split_whitespace().collect();

    let output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg("--diff-filter=ACMR")
        .args(&args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to run git: {} (is git installed?)", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git diff --name-only --diff-filter=ACMR {} failed:\n{}",
            spec,
            stderr.trim()
        )
        .into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: HashSet<PathBuf> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| cwd.join(l))
        .collect();

    Ok(files)
}

/// Filter a list of file paths to only those present in the `changed` set.
/// Paths are canonicalized for reliable comparison.
pub fn intersect_changed(
    files: Vec<String>,
    changed: &HashSet<PathBuf>,
) -> Vec<String> {
    // Pre-canonicalize the changed set for comparison.
    let canonical_changed: HashSet<PathBuf> = changed
        .iter()
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .collect();

    files
        .into_iter()
        .filter(|f| {
            if let Ok(canonical) = std::fs::canonicalize(f) {
                canonical_changed.contains(&canonical)
            } else {
                false
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersect_changed_filters_to_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        let c = dir.path().join("c.rs");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        std::fs::write(&c, "").unwrap();

        let files = vec![
            a.to_str().unwrap().to_string(),
            b.to_str().unwrap().to_string(),
            c.to_str().unwrap().to_string(),
        ];

        // Only a.rs and c.rs are "changed"
        let mut changed = HashSet::new();
        changed.insert(a.clone());
        changed.insert(c.clone());

        let result = intersect_changed(files, &changed);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&a.to_str().unwrap().to_string()));
        assert!(result.contains(&c.to_str().unwrap().to_string()));
    }

    #[test]
    fn intersect_changed_empty_changed_set_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        std::fs::write(&a, "").unwrap();

        let files = vec![a.to_str().unwrap().to_string()];
        let changed = HashSet::new();

        let result = intersect_changed(files, &changed);
        assert!(result.is_empty());
    }

    #[test]
    fn intersect_changed_preserves_order() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        let c = dir.path().join("c.rs");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        std::fs::write(&c, "").unwrap();

        let files = vec![
            c.to_str().unwrap().to_string(),
            a.to_str().unwrap().to_string(),
            b.to_str().unwrap().to_string(),
        ];

        let mut changed = HashSet::new();
        changed.insert(a.clone());
        changed.insert(b.clone());
        changed.insert(c.clone());

        let result = intersect_changed(files.clone(), &changed);
        assert_eq!(result, files);
    }

    #[test]
    fn git_changed_files_in_repo() {
        // This test runs in the tractor repo itself.
        // It verifies that git_changed_files returns successfully for a valid spec.
        let cwd = Path::new(".");
        let result = git_changed_files("HEAD~1 HEAD", cwd);
        // Should succeed (we're in a git repo)
        assert!(result.is_ok(), "git_changed_files should succeed: {:?}", result.err());
    }
}
