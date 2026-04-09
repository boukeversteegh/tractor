//! Helpers for CLI integration tests.
//!
//! Runs the `tractor` binary as a subprocess, avoiding bash entirely.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn tractor_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tractor"))
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

pub fn integration_dir(relative: &str) -> PathBuf {
    repo_root().join("tests/integration").join(relative)
}

pub fn lang_dir(lang: &str) -> PathBuf {
    integration_dir(&format!("languages/{}", lang))
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/// Run `tractor test <args..>` in `dir`, panic if exit != 0.
pub fn tractor_test(dir: &Path, args: &[&str]) {
    let out = Command::new(tractor_bin())
        .current_dir(dir).arg("test").args(args)
        .output().expect("failed to execute tractor");
    if !out.status.success() {
        panic!(
            "tractor test FAILED\n  dir:  {}\n  args: {}\n  stdout: {}\n  stderr: {}",
            dir.display(), args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Run `tractor <args..>` in `dir`, panic if exit == 0.
pub fn tractor_fails(dir: &Path, args: &[&str]) {
    let r = tractor_run(dir, args);
    assert!(!r.success, "tractor should have failed\n  args: {}\n  stdout: {}", args.join(" "), r.stdout);
}

// ---------------------------------------------------------------------------
// Capturing output
// ---------------------------------------------------------------------------

pub struct TractorOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl TractorOutput {
    fn from(o: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
            success: o.status.success(),
        }
    }

    /// Combined stdout + stderr (like `2>&1`).
    pub fn combined(&self) -> String {
        if self.stderr.is_empty() { return self.stdout.clone(); }
        let mut s = self.stdout.clone();
        if !s.is_empty() && !s.ends_with('\n') { s.push('\n'); }
        s.push_str(&self.stderr);
        s
    }
}

pub fn tractor_run(dir: &Path, args: &[&str]) -> TractorOutput {
    TractorOutput::from(
        Command::new(tractor_bin()).current_dir(dir).args(args)
            .output().expect("failed to execute tractor"),
    )
}

pub fn tractor_run_stdin(dir: &Path, args: &[&str], stdin: &str) -> TractorOutput {
    use std::io::Write;
    let mut child = Command::new(tractor_bin())
        .current_dir(dir).args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn().expect("failed to spawn tractor");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    TractorOutput::from(child.wait_with_output().unwrap())
}

pub fn tractor_stdout(dir: &Path, args: &[&str]) -> String {
    let r = tractor_run(dir, args);
    assert!(r.success, "tractor failed\n  args: {}\n  stderr: {}", args.join(" "), r.stderr);
    r.stdout
}

// ---------------------------------------------------------------------------
// Temp-file helpers for set/update tests
// ---------------------------------------------------------------------------

pub struct TempFile {
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

impl TempFile {
    pub fn new(name: &str, content: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        Self { path, _dir: dir }
    }

    pub fn read(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap()
    }

    pub fn path_str(&self) -> String {
        self.path.to_string_lossy().replace('\\', "/")
    }
}

/// Temporary copy of a directory (for tests that modify files in-place).
pub struct TempCopy {
    pub dir: tempfile::TempDir,
}

impl TempCopy {
    pub fn new(source_dir: &Path, extensions: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for entry in std::fs::read_dir(source_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let dominated = extensions.is_empty()
                    || path.extension().map_or(false, |e| extensions.iter().any(|ext| e == *ext));
                if dominated {
                    std::fs::copy(&path, dir.path().join(path.file_name().unwrap())).unwrap();
                }
            }
        }
        Self { dir }
    }

    pub fn path(&self) -> &Path { self.dir.path() }

    pub fn file_str(&self, name: &str) -> String {
        self.dir.path().join(name).to_string_lossy().replace('\\', "/")
    }
}
