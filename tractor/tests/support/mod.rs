#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

#[derive(Clone, Copy, Debug)]
enum StatusExpectation {
    Success,
    Exact(i32),
    Failure,
}

#[derive(Clone, Debug)]
enum ExpectedText {
    Inline(String),
    Snapshot(String),
}

#[derive(Clone, Debug)]
enum TestArg {
    Literal(String),
    FixturePath { relative: String, absolute: bool },
}

#[derive(Clone, Copy, Debug)]
enum OutputKind {
    Stdout,
    Stderr,
    Combined,
}

#[derive(Clone, Debug)]
struct OutputExpectation {
    kind: OutputKind,
    expected: ExpectedText,
}

#[derive(Clone, Debug)]
struct FileExpectation {
    relative: String,
    expected: ExpectedText,
}

#[derive(Clone, Debug)]
struct SeedFile {
    relative: String,
    contents: String,
}

pub struct RunResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub combined: String,
    pub cwd: PathBuf,
    _temp_dir: Option<TempDir>,
}

#[derive(Clone, Debug)]
pub struct CliTest {
    fixture: Option<String>,
    args: Vec<TestArg>,
    stdin: Option<String>,
    status: StatusExpectation,
    outputs: Vec<OutputExpectation>,
    files: Vec<FileExpectation>,
    seed_files: Vec<SeedFile>,
    temp_fixture: bool,
    temp_prefix_replacement: Option<String>,
    fixture_prefix_replacement: Option<String>,
    replacements: Vec<(String, String)>,
    no_color: bool,
}

impl CliTest {
    pub fn new(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            fixture: None,
            args: args
                .into_iter()
                .map(|arg| TestArg::Literal(arg.into()))
                .collect(),
            stdin: None,
            status: StatusExpectation::Success,
            outputs: Vec::new(),
            files: Vec::new(),
            seed_files: Vec::new(),
            temp_fixture: false,
            temp_prefix_replacement: None,
            fixture_prefix_replacement: None,
            replacements: Vec::new(),
            no_color: true,
        }
    }

    pub fn in_fixture(mut self, fixture: impl Into<String>) -> Self {
        self.fixture = Some(fixture.into());
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(TestArg::Literal(arg.into()));
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(|arg| TestArg::Literal(arg.into())));
        self
    }

    pub fn abs_arg(mut self, relative: impl Into<String>) -> Self {
        self.args.push(TestArg::FixturePath {
            relative: relative.into(),
            absolute: true,
        });
        self
    }

    pub fn stdin(mut self, text: impl Into<String>) -> Self {
        self.stdin = Some(text.into());
        self
    }

    pub fn status(mut self, code: i32) -> Self {
        self.status = StatusExpectation::Exact(code);
        self
    }

    pub fn fails(mut self) -> Self {
        self.status = StatusExpectation::Failure;
        self
    }

    pub fn expect(mut self, expected: impl Into<String>) -> Self {
        self = self.arg("--expect");
        self = self.arg(expected);
        self
    }

    pub fn view(mut self, value: impl Into<String>) -> Self {
        self = self.arg("-v");
        self = self.arg(value);
        self
    }

    pub fn format(mut self, value: impl Into<String>) -> Self {
        self = self.arg("-f");
        self = self.arg(value);
        self
    }

    pub fn lang(mut self, value: impl Into<String>) -> Self {
        self = self.arg("--lang");
        self = self.arg(value);
        self
    }

    pub fn tree(mut self, value: impl Into<String>) -> Self {
        self = self.arg("-t");
        self = self.arg(value);
        self
    }

    pub fn reason(mut self, value: impl Into<String>) -> Self {
        self = self.arg("--reason");
        self = self.arg(value);
        self
    }

    pub fn temp_fixture(mut self) -> Self {
        self.temp_fixture = true;
        self
    }

    pub fn strip_temp_prefix(mut self) -> Self {
        self.temp_prefix_replacement = Some(String::new());
        self
    }

    pub fn temp_prefix(mut self, replacement: impl Into<String>) -> Self {
        self.temp_prefix_replacement = Some(replacement.into());
        self
    }

    pub fn fixture_prefix(mut self, replacement: impl Into<String>) -> Self {
        self.fixture_prefix_replacement = Some(replacement.into());
        self
    }

    pub fn replace_output(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        self.replacements.push((from.into(), to.into()));
        self
    }

    pub fn stdout(mut self, expected: impl Into<String>) -> Self {
        self.outputs.push(OutputExpectation {
            kind: OutputKind::Stdout,
            expected: ExpectedText::Inline(expected.into()),
        });
        self
    }

    pub fn stdout_snapshot(mut self, snapshot: impl Into<String>) -> Self {
        self.outputs.push(OutputExpectation {
            kind: OutputKind::Stdout,
            expected: ExpectedText::Snapshot(snapshot.into()),
        });
        self
    }

    pub fn combined(mut self, expected: impl Into<String>) -> Self {
        self.outputs.push(OutputExpectation {
            kind: OutputKind::Combined,
            expected: ExpectedText::Inline(expected.into()),
        });
        self
    }

    pub fn combined_snapshot(mut self, snapshot: impl Into<String>) -> Self {
        self.outputs.push(OutputExpectation {
            kind: OutputKind::Combined,
            expected: ExpectedText::Snapshot(snapshot.into()),
        });
        self
    }

    pub fn file_eq(mut self, relative: impl Into<String>, expected: impl Into<String>) -> Self {
        self.files.push(FileExpectation {
            relative: relative.into(),
            expected: ExpectedText::Inline(expected.into()),
        });
        self
    }

    pub fn file_snapshot(
        mut self,
        relative: impl Into<String>,
        snapshot: impl Into<String>,
    ) -> Self {
        self.files.push(FileExpectation {
            relative: relative.into(),
            expected: ExpectedText::Snapshot(snapshot.into()),
        });
        self
    }

    pub fn seed_file(mut self, relative: impl Into<String>, contents: impl Into<String>) -> Self {
        self.seed_files.push(SeedFile {
            relative: relative.into(),
            contents: contents.into(),
        });
        self
    }

    pub fn capture(self) -> RunResult {
        self.execute(false)
    }

    pub fn run(self) -> RunResult {
        self.execute(true)
    }

    fn execute(self, assert_expectations: bool) -> RunResult {
        let fixture = self.fixture.clone();
        let fixture_root = fixture.as_deref().map(fixture_dir);
        let (cwd, temp_dir) = if self.temp_fixture {
            let source = fixture_root
                .as_ref()
                .expect("temp_fixture requires an attached fixture directory");
            let temp_dir = TempDir::new().expect("failed to create temp fixture directory");
            copy_dir_all(source, temp_dir.path());
            (temp_dir.path().to_path_buf(), Some(temp_dir))
        } else {
            (
                fixture_root.unwrap_or_else(repo_root),
                None,
            )
        };

        for file in &self.seed_files {
            let path = cwd.join(&file.relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("failed to create seed file parent directory");
            }
            fs::write(&path, &file.contents).expect("failed to write seed file");
        }

        let mut command = Command::new(binary_path());
        command.current_dir(&cwd);
        command.stdin(if self.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        for arg in &self.args {
            command.arg(resolve_arg(arg, &cwd));
        }
        if self.no_color {
            command.arg("--no-color");
        }

        let mut child = command.spawn().expect("failed to launch tractor binary");
        if let Some(input) = &self.stdin {
            use std::io::Write;
            let mut stdin = child.stdin.take().expect("failed to open stdin");
            stdin
                .write_all(input.as_bytes())
                .expect("failed to write test stdin");
        }

        let output = child
            .wait_with_output()
            .expect("failed to wait for tractor process");

        let stdout = normalize_text(&String::from_utf8(output.stdout).expect("stdout was not utf-8"));
        let stderr = normalize_text(&String::from_utf8(output.stderr).expect("stderr was not utf-8"));
        let mut result = RunResult {
            status: output.status.code().unwrap_or(-1),
            combined: combine_streams(&stdout, &stderr),
            stdout,
            stderr,
            cwd,
            _temp_dir: temp_dir,
        };

        if let Some(replacement) = &self.temp_prefix_replacement {
            result.stdout = replace_path_prefix(&result.stdout, &result.cwd, replacement);
            result.stderr = replace_path_prefix(&result.stderr, &result.cwd, replacement);
            result.combined = replace_path_prefix(&result.combined, &result.cwd, replacement);
        }

        if let Some(replacement) = &self.fixture_prefix_replacement {
            if let Some(fixture) = &fixture {
                let root = fixture_dir(fixture);
                result.stdout = replace_path_prefix(&result.stdout, &root, replacement);
                result.stderr = replace_path_prefix(&result.stderr, &root, replacement);
                result.combined = replace_path_prefix(&result.combined, &root, replacement);
            }
        }

        for (from, to) in &self.replacements {
            result.stdout = result.stdout.replace(from, to);
            result.stderr = result.stderr.replace(from, to);
            result.combined = result.combined.replace(from, to);
        }

        if assert_expectations {
            assert_status(self.status, result.status);

            for output in &self.outputs {
                let actual = match output.kind {
                    OutputKind::Stdout => &result.stdout,
                    OutputKind::Stderr => &result.stderr,
                    OutputKind::Combined => &result.combined,
                };
                let expected = load_expected(&output.expected);
                assert_eq!(expected, actual.as_str(), "unexpected {:?} output", output.kind);
            }

            for file in &self.files {
                let actual = normalize_text(
                    &fs::read_to_string(result.cwd.join(&file.relative))
                        .unwrap_or_else(|_| panic!("failed to read {}", file.relative)),
                );
                let expected = load_expected(&file.expected);
                assert_eq!(expected, actual, "unexpected file contents for {}", file.relative);
            }
        }

        result
    }
}

pub fn case(args: impl IntoIterator<Item = impl Into<String>>) -> CliTest {
    CliTest::new(args)
}

pub fn expect(file: impl Into<String>, xpath: impl Into<String>, expected: impl Into<String>) -> CliTest {
    CliTest::new(["test"])
        .arg(file.into())
        .arg("-x")
        .arg(xpath.into())
        .expect(expected.into())
}

pub fn inline(lang: impl Into<String>, source: impl Into<String>, xpath: impl Into<String>) -> CliTest {
    CliTest::new(["test"])
        .arg("-s")
        .arg(source.into())
        .arg("-l")
        .arg(lang.into())
        .arg("-x")
        .arg(xpath.into())
}

pub fn query(file: impl Into<String>, xpath: impl Into<String>) -> CliTest {
    CliTest::new([file.into()])
        .arg("-x")
        .arg(xpath.into())
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tractor crate should live under repo root")
        .to_path_buf()
}

pub fn integration_root() -> PathBuf {
    repo_root().join("tests").join("integration")
}

pub fn fixture_dir(relative: &str) -> PathBuf {
    integration_root().join(relative)
}

fn binary_path() -> OsString {
    std::env::var_os("CARGO_BIN_EXE_tractor").unwrap_or_else(|| {
        let exe = std::env::current_exe().expect("failed to resolve current test binary");
        let profile_dir = exe
            .parent()
            .and_then(Path::parent)
            .expect("test binary should live under target/<profile>/deps");
        let mut path = profile_dir.join("tractor");
        if cfg!(windows) {
            path.set_extension("exe");
        }
        path.into_os_string()
    })
}

fn resolve_arg(arg: &TestArg, cwd: &Path) -> OsString {
    match arg {
        TestArg::Literal(value) => value.into(),
        TestArg::FixturePath { relative, absolute } => {
            if *absolute {
                cwd.join(relative).into_os_string()
            } else {
                relative.into()
            }
        }
    }
}

fn load_expected(expected: &ExpectedText) -> String {
    match expected {
        ExpectedText::Inline(value) => normalize_text(value),
        ExpectedText::Snapshot(path) => normalize_text(
            &fs::read_to_string(integration_root().join(path))
                .unwrap_or_else(|_| panic!("failed to read snapshot {}", path)),
        ),
    }
}

fn assert_status(expectation: StatusExpectation, actual: i32) {
    match expectation {
        StatusExpectation::Success => assert_eq!(0, actual, "expected success exit code"),
        StatusExpectation::Exact(expected) => {
            assert_eq!(expected, actual, "unexpected exit code")
        }
        StatusExpectation::Failure => {
            assert!(actual != 0, "expected a non-zero exit code, got {actual}")
        }
    }
}

fn normalize_text(value: &str) -> String {
    value.replace("\r\n", "\n").trim_end_matches('\n').to_string()
}

fn combine_streams(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn replace_path_prefix(text: &str, path: &Path, replacement: &str) -> String {
    let mut output = text.to_string();
    let normalized_replacement = replacement.trim_end_matches(['/', '\\']);
    let trailing = if normalized_replacement.is_empty() {
        String::new()
    } else {
        format!("{normalized_replacement}/")
    };

    for prefix in path_variants(path) {
        let trimmed = prefix.trim_end_matches(['/', '\\']);
        output = output.replace(&format!("{trimmed}/"), &trailing);
        output = output.replace(&format!("{trimmed}\\"), &trailing);
        if !normalized_replacement.is_empty() {
            output = output.replace(trimmed, normalized_replacement);
        }
    }

    output
}

fn path_variants(path: &Path) -> Vec<String> {
    let native = path.to_string_lossy().to_string();
    let forward = native.replace('\\', "/");

    if native == forward {
        vec![native]
    } else {
        vec![native, forward]
    }
}

fn copy_dir_all(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("failed to create destination directory");

    for entry in fs::read_dir(source).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let file_type = entry.file_type().expect("failed to inspect directory entry");
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &destination_path);
        } else {
            fs::copy(entry.path(), destination_path).expect("failed to copy fixture file");
        }
    }
}

macro_rules! cli_suite {
    ($module:ident in $fixture:literal { $($name:ident => $case:expr;)+ }) => {
        mod $module {
            use super::*;

            $(
                #[test]
                fn $name() {
                    $case.in_fixture($fixture).run();
                }
            )+
        }
    };
}
