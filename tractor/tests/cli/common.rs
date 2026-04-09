//! Common helpers for CLI integration tests.
//!
//! These run the `tractor` binary as a true black-box subprocess,
//! avoiding bash and its path-mangling issues on Windows/WSL.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the compiled `tractor` binary (built by cargo before tests run).
pub fn tractor_bin() -> PathBuf {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests
    PathBuf::from(env!("CARGO_BIN_EXE_tractor"))
}

/// Repository root (parent of the `tractor` crate directory).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tractor crate should be inside repo root")
        .to_path_buf()
}

/// Resolve a path relative to `tests/integration/`.
pub fn integration_dir(relative: &str) -> PathBuf {
    repo_root().join("tests/integration").join(relative)
}

/// Shorthand for `integration_dir("languages/<lang>")`.
pub fn lang_dir(lang: &str) -> PathBuf {
    integration_dir(&format!("languages/{}", lang))
}

// ---------------------------------------------------------------------------
// Running `tractor test` (exit-code based assertions)
// ---------------------------------------------------------------------------

/// Run `tractor test <args..>` in `dir` and assert exit code 0.
pub fn tractor_test(dir: &Path, args: &[&str]) {
    let output = Command::new(tractor_bin())
        .current_dir(dir)
        .arg("test")
        .args(args)
        .output()
        .expect("failed to execute tractor");

    if !output.status.success() {
        panic!(
            "tractor test FAILED\n  dir:    {}\n  args:   {}\n  stdout: {}\n  stderr: {}",
            dir.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

/// Run `tractor test <args..>` in `dir` and assert exit code != 0.
#[allow(dead_code)]
pub fn tractor_test_fails(dir: &Path, args: &[&str]) {
    let output = Command::new(tractor_bin())
        .current_dir(dir)
        .arg("test")
        .args(args)
        .output()
        .expect("failed to execute tractor");

    if output.status.success() {
        panic!(
            "tractor test should have FAILED but succeeded\n  dir:    {}\n  args:   {}\n  stdout: {}",
            dir.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
        );
    }
}

// ---------------------------------------------------------------------------
// Running arbitrary tractor subcommands
// ---------------------------------------------------------------------------

/// Captured result of a tractor invocation.
pub struct TractorOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    #[allow(dead_code)]
    pub code: Option<i32>,
}

impl TractorOutput {
    pub fn from(output: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            success: output.status.success(),
            code: output.status.code(),
        }
    }

    /// Combined stdout + stderr (like shell `2>&1`).
    pub fn combined(&self) -> String {
        let mut s = self.stdout.clone();
        if !self.stderr.is_empty() {
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
            s.push_str(&self.stderr);
        }
        s
    }
}

/// Run `tractor <args..>` in `dir` and capture output.
pub fn tractor_run(dir: &Path, args: &[&str]) -> TractorOutput {
    let output = Command::new(tractor_bin())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to execute tractor");
    TractorOutput::from(output)
}

/// Run `tractor <args..>` in `dir` with stdin piped in.
pub fn tractor_run_stdin(dir: &Path, args: &[&str], stdin: &str) -> TractorOutput {
    use std::io::Write;
    let mut child = Command::new(tractor_bin())
        .current_dir(dir)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn tractor");

    if let Some(ref mut w) = child.stdin {
        w.write_all(stdin.as_bytes()).expect("failed to write stdin");
    }
    drop(child.stdin.take()); // close stdin

    let output = child.wait_with_output().expect("failed to wait on tractor");
    TractorOutput::from(output)
}

/// Run `tractor <args..>` in `dir`, assert success, return stdout.
#[allow(dead_code)]
pub fn tractor_stdout(dir: &Path, args: &[&str]) -> String {
    let r = tractor_run(dir, args);
    assert!(
        r.success,
        "tractor failed\n  dir:  {}\n  args: {}\n  stderr: {}",
        dir.display(),
        args.join(" "),
        r.stderr,
    );
    r.stdout
}

/// Run `tractor <args..>` in `dir`, assert failure.
pub fn tractor_fails(dir: &Path, args: &[&str]) {
    let r = tractor_run(dir, args);
    assert!(
        !r.success,
        "tractor should have failed\n  dir:  {}\n  args: {}\n  stdout: {}",
        dir.display(),
        args.join(" "),
        r.stdout,
    );
}

// ---------------------------------------------------------------------------
// Temp-file helpers for set/update tests
// ---------------------------------------------------------------------------

/// Write `content` to a temp file with the given extension, return its path.
/// The returned `TempFile` keeps the file alive until dropped.
pub struct TempFile {
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

impl TempFile {
    pub fn new(name: &str, content: &str) -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join(name);
        std::fs::write(&path, content).expect("failed to write temp file");
        Self { path, _dir: dir }
    }

    /// Read the current file contents.
    pub fn read(&self) -> String {
        std::fs::read_to_string(&self.path).expect("failed to read temp file")
    }

    /// Get the path as a string (using forward slashes for tractor compatibility).
    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().replace('\\', "/")
    }
}

/// Create a temporary copy of a directory for tests that modify files.
pub struct TempCopy {
    pub dir: tempfile::TempDir,
}

impl TempCopy {
    /// Copy all files matching `extensions` from `source_dir` into a new temp dir.
    pub fn new(source_dir: &Path, extensions: &[&str]) -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        for entry in std::fs::read_dir(source_dir).expect("failed to read source dir") {
            let entry = entry.expect("failed to read dir entry");
            let path = entry.path();
            if path.is_file() {
                let dominated = extensions.is_empty()
                    || path
                        .extension()
                        .map_or(false, |e| extensions.iter().any(|ext| e == *ext));
                if dominated {
                    let dest = dir.path().join(path.file_name().unwrap());
                    std::fs::copy(&path, &dest).expect("failed to copy file");
                }
            }
        }
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Read a file inside the temp dir.
    #[allow(dead_code)]
    pub fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(name)).expect("failed to read file")
    }

    /// Get a file path string with forward slashes.
    pub fn file_str(&self, name: &str) -> String {
        self.dir
            .path()
            .join(name)
            .to_string_lossy()
            .replace('\\', "/")
    }
}
