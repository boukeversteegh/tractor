//! Git integration for filtering files by change status.
//!
//! Resolves changed files by invoking `git diff --name-only` with a
//! user-provided diff specification (e.g. `HEAD~3`, `main..HEAD`).
//!
//! Also provides `DiffHunkFilter` for line-level filtering: only keep
//! matches whose line ranges overlap with changed hunks in a git diff.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use tractor::{Match, NormalizedPath};

/// Run `git diff --name-only` with the given spec and return the set of
/// changed file paths, resolved relative to `cwd`.
///
/// The spec is split on whitespace and passed as separate arguments to git,
/// so `"main..HEAD"` becomes `["main..HEAD"]` and `"HEAD -- src/"` becomes
/// `["HEAD", "--", "src/"]`.
///
/// Deleted files are excluded via `--diff-filter=ACMR` (Added, Copied,
/// Modified, Renamed).
///
/// Paths are resolved via [`NormalizedPath::absolute`], which lexically
/// joins and case-corrects but does **not** follow symlinks. This matches
/// the glob walker's behavior so the two sets can intersect as-is.
pub fn git_changed_files(
    spec: &str,
    cwd: &Path,
) -> Result<HashSet<NormalizedPath>, Box<dyn std::error::Error>> {
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
    let files: HashSet<NormalizedPath> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| NormalizedPath::absolute(&cwd.join(l).to_string_lossy()))
        .collect();

    Ok(files)
}

/// Filter a list of file paths to only those present in the `changed` set.
///
/// Both sides are already `NormalizedPath`, produced by
/// [`NormalizedPath::absolute`] at their respective entry points. That
/// alignment is what makes this a simple hash-set lookup — no canonicalize,
/// no symlink following.
pub fn intersect_changed(
    files: Vec<NormalizedPath>,
    changed: &HashSet<NormalizedPath>,
) -> Vec<NormalizedPath> {
    files.into_iter().filter(|f| changed.contains(f)).collect()
}

// ---------------------------------------------------------------------------
// DiffHunkFilter — line-level filtering based on git diff hunks
// ---------------------------------------------------------------------------

/// A line range in a file (1-based, inclusive on both ends).
#[derive(Debug, Clone, PartialEq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

/// Filters matches to only those whose line ranges overlap with changed
/// hunks from a git diff. Also supports file-level filtering: files
/// not present in the diff are skipped entirely.
#[derive(Debug, Clone)]
pub struct DiffHunkFilter {
    /// Map from normalized absolute file path → changed line ranges.
    hunks: HashMap<NormalizedPath, Vec<LineRange>>,
}

impl DiffHunkFilter {
    /// Create a filter from a git diff spec (e.g. `"HEAD~3"`, `"main..HEAD"`).
    ///
    /// Runs `git diff -U0 <spec>` and parses the unified diff output to
    /// extract changed line ranges on the new side of the diff.
    pub fn from_spec(spec: &str, cwd: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let diff_output = run_git_diff(spec, cwd)?;
        let hunks = parse_diff_hunks(&diff_output, cwd);
        Ok(DiffHunkFilter { hunks })
    }

    /// Create a filter that also covers an inline source at a virtual path.
    ///
    /// For real files, hunks come from `git diff -U0 <spec>` as usual.
    /// For the virtual path, hunks are computed as `diff -U0 <git show
    /// <spec>:<vpath>, inline content>` — the "new side" is the in-memory
    /// content, so the resulting ranges reflect the caller's proposed edit.
    ///
    /// This is what lets pre-commit hooks lint only the hunk they're
    /// actually changing, even though the stdin content never hits disk.
    pub fn from_spec_with_inline(
        spec: &str,
        cwd: &Path,
        inline: Option<(&NormalizedPath, &str)>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut filter = Self::from_spec(spec, cwd)?;
        if let Some((vpath, content)) = inline {
            let baseline = git_show_at_spec(spec, vpath, cwd).unwrap_or_default();
            let ranges = compute_inline_hunks(&baseline, content)?;
            if ranges.is_empty() {
                filter.hunks.remove(vpath);
            } else {
                filter.hunks.insert(vpath.clone(), ranges);
            }
        }
        Ok(filter)
    }

    /// Create a filter from pre-parsed hunks (for testing).
    #[cfg(test)]
    pub fn from_hunks(hunks: HashMap<NormalizedPath, Vec<LineRange>>) -> Self {
        DiffHunkFilter { hunks }
    }
}

/// Read the baseline content of `path` at git `spec`.
///
/// Uses `git show <spec>:<repo-relative-path>`. Returns `None` if the file
/// doesn't exist at the spec (new file) or git rejects the path — the
/// caller should treat that as an empty baseline so every line counts as
/// added.
fn git_show_at_spec(spec: &str, path: &NormalizedPath, cwd: &Path) -> Option<String> {
    // `git show SPEC:PATH` wants a single revision, not a range. If spec is
    // a range (`A..B`, `A...B`, or `A B`), take the last revision — it's
    // the "new" side that the caller wants to compare against.
    let revision = last_revision(spec);

    // git show wants a repo-relative path. Derive it from the repo root.
    let repo_root = repo_toplevel(cwd)?;
    let rel = path.as_str().strip_prefix(repo_root.as_str())?
        .trim_start_matches('/').to_string();

    let output = Command::new("git")
        .arg("show")
        .arg(format!("{}:{}", revision, rel))
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn last_revision(spec: &str) -> &str {
    // Handle "A B" (pair) or "A..B" / "A...B" (range): take the last token.
    let trimmed = spec.trim();
    if let Some(idx) = trimmed.rfind("...") {
        return &trimmed[idx + 3..];
    }
    if let Some(idx) = trimmed.rfind("..") {
        return &trimmed[idx + 2..];
    }
    trimmed.split_whitespace().last().unwrap_or(trimmed)
}

fn repo_toplevel(cwd: &Path) -> Option<NormalizedPath> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(NormalizedPath::absolute(&raw))
}

/// Compute line ranges that changed going from `baseline` to `content`.
///
/// Shells out to `diff -U0 <base_file> <new_file>` on two temp files and
/// parses the unified diff hunk headers. Temp files are written to the
/// system temp dir and removed on the way out; we avoid pulling in the
/// `tempfile` crate as a regular dependency since this helper is only
/// used on the inline-source path.
///
/// Returns hunk ranges on the new side (line numbers in `content`).
fn compute_inline_hunks(
    baseline: &str,
    content: &str,
) -> Result<Vec<LineRange>, Box<dyn std::error::Error>> {
    let base_file = write_scratch_file("tractor_diff_base", baseline)?;
    let new_file = write_scratch_file("tractor_diff_new", content)?;

    let result = (|| -> Result<Vec<LineRange>, Box<dyn std::error::Error>> {
        let output = Command::new("diff")
            .arg("-U0")
            .arg(&base_file)
            .arg(&new_file)
            .output()?;

        // `diff -U0` exits 0 when files are identical, 1 when different,
        // >1 on error. 0 and 1 both yield parseable output; >1 is a real
        // failure.
        if let Some(code) = output.status.code() {
            if code > 1 {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("diff -U0 failed: {}", stderr.trim()).into());
            }
        }

        let diff = String::from_utf8_lossy(&output.stdout);
        let mut ranges = Vec::new();
        for line in diff.lines() {
            if line.starts_with("@@ ") {
                if let Some(range) = parse_hunk_header(line) {
                    ranges.push(range);
                }
            }
        }
        Ok(ranges)
    })();

    let _ = std::fs::remove_file(&base_file);
    let _ = std::fs::remove_file(&new_file);
    result
}

/// Create a short-lived scratch file in the OS temp dir. Unique per-call
/// via PID + nanosecond timestamp + a monotonic counter.
fn write_scratch_file(tag: &str, content: &str) -> std::io::Result<std::path::PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);

    let path = std::env::temp_dir().join(format!("{}_{}_{}_{}", tag, pid, nanos, seq));
    std::fs::write(&path, content)?;
    Ok(path)
}

impl DiffHunkFilter {
    /// Returns true if this match overlaps one of the changed hunks.
    pub fn include(&self, m: &Match) -> bool {
        // `Match::file` originates from the file resolver (already absolute
        // + normalized), so wrapping it as `NormalizedPath` without going
        // through `absolute()` again is safe and cheap.
        let path = NormalizedPath::new(&m.file);
        match self.hunks.get(&path) {
            Some(ranges) => ranges.iter().any(|r| {
                // Overlap: match.line <= range.end && match.end_line >= range.start
                m.line <= r.end && m.end_line >= r.start
            }),
            None => false, // file not in diff → exclude
        }
    }

    /// Returns true if the file is touched by the diff at all.
    pub fn include_file(&self, file: &str) -> bool {
        self.hunks.contains_key(&NormalizedPath::new(file))
    }
}

/// Run `git diff -U0` and return the raw output.
fn run_git_diff(spec: &str, cwd: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let args: Vec<&str> = spec.split_whitespace().collect();

    let output = Command::new("git")
        .arg("diff")
        .arg("-U0")
        .arg("--diff-filter=ACMR")
        .args(&args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to run git: {} (is git installed?)", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff -U0 {} failed:\n{}", spec, stderr.trim()).into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse unified diff output into per-file hunk line ranges.
///
/// Looks for:
/// - `diff --git a/... b/<path>` to identify the file
/// - `@@ -old_start[,old_count] +new_start[,new_count] @@` for hunk ranges
///
/// Only new-side ranges are extracted. Pure deletions (new_count == 0) are
/// skipped since there are no new lines to match against.
fn parse_diff_hunks(diff: &str, cwd: &Path) -> HashMap<NormalizedPath, Vec<LineRange>> {
    let mut hunks: HashMap<NormalizedPath, Vec<LineRange>> = HashMap::new();
    let mut current_file: Option<NormalizedPath> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // Extract the b/ path: "a/foo b/bar" → "bar"
            if let Some(b_idx) = rest.rfind(" b/") {
                let rel_path = &rest[b_idx + 3..];
                let abs_path = cwd.join(rel_path);
                current_file = Some(NormalizedPath::absolute(&abs_path.to_string_lossy()));
            }
        } else if line.starts_with("@@ ") {
            if let Some(ref file) = current_file {
                if let Some(range) = parse_hunk_header(line) {
                    hunks.entry(file.clone()).or_default().push(range);
                }
            }
        }
    }

    hunks
}

/// Parse a unified diff hunk header like `@@ -10,3 +15,5 @@ context`.
///
/// Returns the new-side line range, or None for pure deletions (count == 0).
fn parse_hunk_header(line: &str) -> Option<LineRange> {
    // Format: @@ -old_start[,old_count] +new_start[,new_count] @@
    let plus_idx = line.find('+')?;
    let after_plus = &line[plus_idx + 1..];
    let end_idx = after_plus.find(' ').unwrap_or(after_plus.len());
    let range_str = &after_plus[..end_idx];

    let (start, count) = if let Some(comma_idx) = range_str.find(',') {
        let start: u32 = range_str[..comma_idx].parse().ok()?;
        let count: u32 = range_str[comma_idx + 1..].parse().ok()?;
        (start, count)
    } else {
        let start: u32 = range_str.parse().ok()?;
        (start, 1) // no comma means exactly 1 line
    };

    if count == 0 {
        return None; // pure deletion, no new lines
    }

    Some(LineRange {
        start,
        end: start + count - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: wrap a filesystem path as a `NormalizedPath` via `absolute()`.
    fn np(path: &std::path::Path) -> NormalizedPath {
        NormalizedPath::absolute(&path.to_string_lossy())
    }

    #[test]
    fn intersect_changed_filters_to_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        let c = dir.path().join("c.rs");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        std::fs::write(&c, "").unwrap();

        let files = vec![np(&a), np(&b), np(&c)];

        // Only a.rs and c.rs are "changed"
        let mut changed = HashSet::new();
        changed.insert(np(&a));
        changed.insert(np(&c));

        let result = intersect_changed(files, &changed);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&np(&a)));
        assert!(result.contains(&np(&c)));
    }

    #[test]
    fn intersect_changed_empty_changed_set_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        std::fs::write(&a, "").unwrap();

        let files = vec![np(&a)];
        let changed = HashSet::new();

        let result = intersect_changed(files, &changed);
        assert!(result.is_empty());
    }

    /// Regression: a symlink path must intersect with itself, not be
    /// routed through `canonicalize` (which would resolve to the target).
    /// Both the CLI/walker side and the diff side are `NormalizedPath`
    /// obtained via `absolute()`, which deliberately does not follow
    /// symlinks — so the intersection hits the link path directly.
    #[cfg(unix)]
    #[test]
    fn intersect_changed_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.rs");
        let link = dir.path().join("link.rs");
        std::fs::write(&target, "").unwrap();
        symlink(&target, &link).unwrap();

        let files = vec![np(&link)];
        let mut changed = HashSet::new();
        changed.insert(np(&link));

        let result = intersect_changed(files, &changed);
        assert_eq!(result.len(), 1);
        assert!(
            result[0].as_str().ends_with("/link.rs"),
            "intersection must stay on the symlink path, not resolve to the target; got: {}",
            result[0]
        );
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

        let files = vec![np(&c), np(&a), np(&b)];

        let mut changed = HashSet::new();
        changed.insert(np(&a));
        changed.insert(np(&b));
        changed.insert(np(&c));

        let result = intersect_changed(files.clone(), &changed);
        assert_eq!(result, files);
    }

    /// Helper: create a temp git repo with an initial commit, return the dir.
    fn init_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .env("GIT_AUTHOR_NAME", "test")
                .env("GIT_AUTHOR_EMAIL", "test@test")
                .env("GIT_COMMITTER_NAME", "test")
                .env("GIT_COMMITTER_EMAIL", "test@test")
                .output()
                .unwrap();
            assert!(output.status.success(), "git {:?} failed: {}",
                args, String::from_utf8_lossy(&output.stderr));
        };

        run(&["init"]);
        run(&["-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "initial"]);
        dir
    }

    #[test]
    fn git_changed_files_detects_changed_file() {
        let dir = init_test_repo();
        let cwd = dir.path();

        // Create a file and commit it
        std::fs::write(cwd.join("a.rs"), "fn main() {}").unwrap();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(cwd)
                .env("GIT_AUTHOR_NAME", "test").env("GIT_AUTHOR_EMAIL", "test@test")
                .env("GIT_COMMITTER_NAME", "test").env("GIT_COMMITTER_EMAIL", "test@test")
                .output().unwrap();
        };
        run(&["add", "a.rs"]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "add a.rs"]);

        // Modify the file and commit
        std::fs::write(cwd.join("a.rs"), "fn main() { println!(\"hello\"); }").unwrap();
        std::fs::write(cwd.join("b.rs"), "fn other() {}").unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "modify a.rs, add b.rs"]);

        let changed = git_changed_files("HEAD~1 HEAD", cwd).unwrap();
        let a_path = np(&cwd.join("a.rs"));
        let b_path = np(&cwd.join("b.rs"));

        assert!(changed.contains(&a_path), "a.rs should be in changed set");
        assert!(changed.contains(&b_path), "b.rs should be in changed set");
        assert_eq!(changed.len(), 2);
    }

    #[test]
    fn diff_hunk_filter_from_spec_with_real_repo() {
        let dir = init_test_repo();
        let cwd = dir.path();

        // Create a file with 10 lines and commit
        let original = (1..=10).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        std::fs::write(cwd.join("test.rs"), &original).unwrap();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(cwd)
                .env("GIT_AUTHOR_NAME", "test").env("GIT_AUTHOR_EMAIL", "test@test")
                .env("GIT_COMMITTER_NAME", "test").env("GIT_COMMITTER_EMAIL", "test@test")
                .output().unwrap();
        };
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "initial file"]);

        // Modify lines 3 and 7, commit
        let modified = original
            .replace("line 3", "line 3 CHANGED")
            .replace("line 7", "line 7 CHANGED");
        std::fs::write(cwd.join("test.rs"), &modified).unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "modify lines 3 and 7"]);

        let filter = DiffHunkFilter::from_spec("HEAD~1 HEAD", cwd).unwrap();
        let test_norm = np(&cwd.join("test.rs"));

        // File should be included
        assert!(filter.include_file(test_norm.as_str()));

        // Match on line 3 should be included
        let m = Match::new(test_norm.as_str().to_string(), "x".into());
        let m3 = Match { line: 3, end_line: 3, ..m.clone() };
        assert!(filter.include(&m3), "line 3 should be in a changed hunk");

        // Match on line 7 should be included
        let m7 = Match { line: 7, end_line: 7, ..m.clone() };
        assert!(filter.include(&m7), "line 7 should be in a changed hunk");

        // Match on line 5 (unchanged) should be excluded
        let m5 = Match { line: 5, end_line: 5, ..m };
        assert!(!filter.include(&m5), "line 5 should NOT be in a changed hunk");
    }

    // -----------------------------------------------------------------------
    // Hunk header parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_hunk_header_standard() {
        let range = parse_hunk_header("@@ -10,3 +15,5 @@ fn foo()").unwrap();
        assert_eq!(range, LineRange { start: 15, end: 19 });
    }

    #[test]
    fn parse_hunk_header_single_line() {
        let range = parse_hunk_header("@@ -10,1 +15,1 @@").unwrap();
        assert_eq!(range, LineRange { start: 15, end: 15 });
    }

    #[test]
    fn parse_hunk_header_no_comma_means_one_line() {
        let range = parse_hunk_header("@@ -10 +15 @@").unwrap();
        assert_eq!(range, LineRange { start: 15, end: 15 });
    }

    #[test]
    fn parse_hunk_header_pure_deletion_returns_none() {
        let range = parse_hunk_header("@@ -10,3 +15,0 @@");
        assert!(range.is_none());
    }

    #[test]
    fn parse_hunk_header_pure_addition() {
        let range = parse_hunk_header("@@ -10,0 +11,4 @@").unwrap();
        assert_eq!(range, LineRange { start: 11, end: 14 });
    }

    // -----------------------------------------------------------------------
    // parse_diff_hunks
    // -----------------------------------------------------------------------

    #[test]
    fn parse_diff_hunks_multiple_files_and_hunks() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();

        let diff = format!(
            "diff --git a/a.rs b/a.rs\n\
             --- a/a.rs\n\
             +++ b/a.rs\n\
             @@ -1,2 +1,3 @@\n\
             +added line\n\
             @@ -10,0 +11,2 @@\n\
             +another\n\
             +addition\n\
             diff --git a/b.rs b/b.rs\n\
             --- a/b.rs\n\
             +++ b/b.rs\n\
             @@ -5,1 +5,1 @@\n\
             -old\n\
             +new\n"
        );

        let hunks = parse_diff_hunks(&diff, dir.path());

        let a_norm = np(&a);
        let b_norm = np(&b);

        let a_hunks = hunks.get(&a_norm).unwrap();
        assert_eq!(a_hunks.len(), 2);
        assert_eq!(a_hunks[0], LineRange { start: 1, end: 3 });
        assert_eq!(a_hunks[1], LineRange { start: 11, end: 12 });

        let b_hunks = hunks.get(&b_norm).unwrap();
        assert_eq!(b_hunks.len(), 1);
        assert_eq!(b_hunks[0], LineRange { start: 5, end: 5 });
    }

    // -----------------------------------------------------------------------
    // DiffHunkFilter include/include_file
    // -----------------------------------------------------------------------

    #[test]
    fn diff_hunk_filter_include_match_in_hunk() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        std::fs::write(&a, "line1\nline2\nline3\nline4\nline5\n").unwrap();
        let a_norm = np(&a);

        let mut hunks = HashMap::new();
        hunks.insert(a_norm.clone(), vec![LineRange { start: 2, end: 4 }]);
        let filter = DiffHunkFilter::from_hunks(hunks);

        // Match fully within hunk. `Match::file` must already be normalized —
        // that's the invariant the file resolver provides in production.
        let m = Match::new(a_norm.as_str().to_string(), "x".into());
        let m = Match { line: 3, end_line: 3, ..m };
        assert!(filter.include(&m));

        // Match overlapping hunk start
        let m = Match { line: 1, end_line: 2, ..m };
        assert!(filter.include(&m));

        // Match overlapping hunk end
        let m = Match { line: 4, end_line: 6, ..m };
        assert!(filter.include(&m));

        // Match completely outside hunk
        let m = Match { line: 5, end_line: 6, ..m };
        assert!(!filter.include(&m));
    }

    #[test]
    fn diff_hunk_filter_exclude_file_not_in_diff() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        let a_norm = np(&a);

        let mut hunks = HashMap::new();
        hunks.insert(a_norm.clone(), vec![LineRange { start: 1, end: 10 }]);
        let filter = DiffHunkFilter::from_hunks(hunks);

        assert!(filter.include_file(a_norm.as_str()));
        assert!(!filter.include_file(np(&b).as_str()));
    }

    #[test]
    fn git_changed_files_excludes_deleted_files() {
        let dir = init_test_repo();
        let cwd = dir.path();

        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(cwd)
                .env("GIT_AUTHOR_NAME", "test").env("GIT_AUTHOR_EMAIL", "test@test")
                .env("GIT_COMMITTER_NAME", "test").env("GIT_COMMITTER_EMAIL", "test@test")
                .output().unwrap();
        };

        // Create two files and commit
        std::fs::write(cwd.join("keep.rs"), "keep").unwrap();
        std::fs::write(cwd.join("delete.rs"), "delete").unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "add files"]);

        // Delete one file, commit
        std::fs::remove_file(cwd.join("delete.rs")).unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "delete file"]);

        let changed = git_changed_files("HEAD~1 HEAD", cwd).unwrap();

        // delete.rs should NOT appear (--diff-filter=ACMR excludes deletions)
        assert!(changed.is_empty(), "deleted files should be excluded, got: {:?}", changed);
    }
}
