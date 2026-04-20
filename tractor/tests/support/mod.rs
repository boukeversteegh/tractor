#![allow(dead_code)]

//! Shared support for the CLI integration DSL.
//!
//! Design rationale lives in `docs/design-cli-integration-dsl.md`. This module
//! keeps the same layers: suite structure, command capture, assertion parsing,
//! and execution.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Layer 2: command capture
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum CommandArg {
    Literal(String),
    FixturePath { relative: String, absolute: bool },
}

/// A captured `tractor ...` invocation.
///
/// This stores only CLI-shaped process inputs: arguments, stdin, and the
/// default `--no-color` test setting. Fixture copying, seeded files, and
/// output normalization live in the harness setup instead.
#[derive(Clone, Debug)]
pub struct TractorInvocation {
    args: Vec<CommandArg>,
    stdin: Option<String>,
    no_color: bool,
}

impl TractorInvocation {
    pub fn from_dsl(command: &str) -> Self {
        Self::new(parse_dsl_words(command))
    }

    pub fn new(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            args: args
                .into_iter()
                .map(|arg| CommandArg::Literal(arg.into()))
                .collect(),
            stdin: None,
            no_color: true,
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(CommandArg::Literal(arg.into()));
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args
            .extend(args.into_iter().map(|arg| CommandArg::Literal(arg.into())));
        self
    }

    pub fn abs_arg(mut self, relative: impl Into<String>) -> Self {
        self.args.push(CommandArg::FixturePath {
            relative: relative.into(),
            absolute: true,
        });
        self
    }

    pub fn stdin(mut self, text: impl Into<String>) -> Self {
        self.stdin = Some(text.into());
        self
    }

    pub fn no_color(mut self, enabled: bool) -> Self {
        self.no_color = enabled;
        self
    }

    fn with_count_view(&self) -> Self {
        let mut args = Vec::new();
        let mut iter = self.args.iter().cloned();

        while let Some(arg) = iter.next() {
            let literal = match &arg {
                CommandArg::Literal(value) => Some(value.as_str()),
                CommandArg::FixturePath { .. } => None,
            };

            match literal {
                Some("-v") | Some("--view") => {
                    let _ = iter.next();
                }
                Some("-f") | Some("--format") => {
                    let _ = iter.next();
                }
                Some("-p") | Some("--projection") | Some("--project") => {
                    let _ = iter.next();
                }
                Some(value) if value.starts_with("-v=") || value.starts_with("--view=") => {}
                Some(value) if value.starts_with("-f=") || value.starts_with("--format=") => {}
                Some(value)
                    if value.starts_with("-p=")
                        || value.starts_with("--projection=")
                        || value.starts_with("--project=") => {}
                _ => args.push(arg),
            }
        }

        let mut invocation = Self {
            args,
            stdin: self.stdin.clone(),
            no_color: self.no_color,
        };
        invocation.args.push(CommandArg::Literal("-v".to_string()));
        invocation
            .args
            .push(CommandArg::Literal("count".to_string()));
        invocation.args.push(CommandArg::Literal("-p".to_string()));
        invocation
            .args
            .push(CommandArg::Literal("count".to_string()));
        invocation
    }
}

// ---------------------------------------------------------------------------
// Layer 3: assertion parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) enum ExpectedText {
    Inline(String),
    Snapshot(String),
}

#[derive(Clone, Debug)]
pub(crate) enum CountExpectation {
    Exact(usize),
    Some,
    None,
}

/// Harness-side assertions that run after the command is captured.
///
/// These are deliberately named as test assertions, not as Tractor flags.
/// They describe how the harness should evaluate the outcome of the command.
#[derive(Clone, Debug)]
pub(crate) enum Assertion {
    Exit(i32),
    Count(CountExpectation),
    Stdout(ExpectedText),
    Stderr(ExpectedText),
    Combined(ExpectedText),
    StdoutContains(String),
    StderrContains(String),
    CombinedContains(String),
    FileEq {
        relative: String,
        expected: ExpectedText,
    },
    FileContains {
        relative: String,
        needle: String,
    },
}

impl Assertion {
    pub fn exit(code: i32) -> Self {
        Self::Exit(code)
    }

    pub fn count(expected: usize) -> Self {
        Self::Count(CountExpectation::Exact(expected))
    }

    pub fn count_some() -> Self {
        Self::Count(CountExpectation::Some)
    }

    pub fn count_none() -> Self {
        Self::Count(CountExpectation::None)
    }

    pub fn stdout(expected: impl Into<String>) -> Self {
        Self::Stdout(ExpectedText::Inline(expected.into()))
    }

    pub fn stdout_snapshot(snapshot: impl Into<String>) -> Self {
        Self::Stdout(ExpectedText::Snapshot(snapshot.into()))
    }

    pub fn stderr(expected: impl Into<String>) -> Self {
        Self::Stderr(ExpectedText::Inline(expected.into()))
    }

    pub fn stderr_snapshot(snapshot: impl Into<String>) -> Self {
        Self::Stderr(ExpectedText::Snapshot(snapshot.into()))
    }

    pub fn combined(expected: impl Into<String>) -> Self {
        Self::Combined(ExpectedText::Inline(expected.into()))
    }

    pub fn combined_snapshot(snapshot: impl Into<String>) -> Self {
        Self::Combined(ExpectedText::Snapshot(snapshot.into()))
    }

    pub fn stdout_contains(needle: impl Into<String>) -> Self {
        Self::StdoutContains(needle.into())
    }

    pub fn stderr_contains(needle: impl Into<String>) -> Self {
        Self::StderrContains(needle.into())
    }

    pub fn combined_contains(needle: impl Into<String>) -> Self {
        Self::CombinedContains(needle.into())
    }

    pub fn file_eq(relative: impl Into<String>, expected: impl Into<String>) -> Self {
        Self::FileEq {
            relative: relative.into(),
            expected: ExpectedText::Inline(expected.into()),
        }
    }

    pub fn file_snapshot(relative: impl Into<String>, snapshot: impl Into<String>) -> Self {
        Self::FileEq {
            relative: relative.into(),
            expected: ExpectedText::Snapshot(snapshot.into()),
        }
    }

    pub fn file_contains(relative: impl Into<String>, needle: impl Into<String>) -> Self {
        Self::FileContains {
            relative: relative.into(),
            needle: needle.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Layer 4: execution
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct HarnessSetup {
    fixture: Option<String>,
    temp_fixture: bool,
    temp_prefix_replacement: Option<String>,
    fixture_prefix_replacement: Option<String>,
    replacements: Vec<(String, String)>,
    seed_files: Vec<SeedFile>,
}

#[derive(Clone, Debug)]
struct SeedFile {
    relative: String,
    contents: String,
}

struct ExecutionContext {
    cwd: PathBuf,
    fixture: Option<String>,
    temp_dir: Option<TempDir>,
}

pub struct RunResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub combined: String,
    pub cwd: PathBuf,
    _temp_dir: Option<TempDir>,
}

/// A complete CLI integration case.
///
/// The harness model is explicit: a Tractor invocation plus harness assertions.
#[derive(Clone, Debug)]
pub struct TestCase {
    command: TractorInvocation,
    assertions: Vec<Assertion>,
    setup: HarnessSetup,
}

impl TestCase {
    pub fn new(command: TractorInvocation) -> Self {
        Self {
            command,
            assertions: Vec::new(),
            setup: HarnessSetup::default(),
        }
    }

    pub fn with_assertion(mut self, assertion: Assertion) -> Self {
        self.assertions.push(assertion);
        self
    }

    pub fn with_assertions(mut self, assertions: impl IntoIterator<Item = Assertion>) -> Self {
        self.assertions.extend(assertions);
        self
    }

    pub fn in_fixture(mut self, fixture: impl Into<String>) -> Self {
        self.setup.fixture = Some(fixture.into());
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.command = self.command.arg(arg);
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.command = self.command.args(args);
        self
    }

    pub fn abs_arg(mut self, relative: impl Into<String>) -> Self {
        self.command = self.command.abs_arg(relative);
        self
    }

    pub fn stdin(mut self, text: impl Into<String>) -> Self {
        self.command = self.command.stdin(text);
        self
    }

    pub fn temp_fixture(mut self) -> Self {
        self.setup.temp_fixture = true;
        self
    }

    /// Opt out of the implicit `--no-color` flag. Useful for subcommands that
    /// don't accept it (e.g. `init`) or when a test wants to assert that color
    /// output is produced.
    pub fn no_color(mut self, enabled: bool) -> Self {
        self.command = self.command.no_color(enabled);
        self
    }

    pub fn strip_temp_prefix(mut self) -> Self {
        self.setup.temp_prefix_replacement = Some(String::new());
        self
    }

    pub fn temp_prefix(mut self, replacement: impl Into<String>) -> Self {
        self.setup.temp_prefix_replacement = Some(replacement.into());
        self
    }

    pub fn fixture_prefix(mut self, replacement: impl Into<String>) -> Self {
        self.setup.fixture_prefix_replacement = Some(replacement.into());
        self
    }

    pub fn replace_output(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.setup.replacements.push((from.into(), to.into()));
        self
    }

    pub fn seed_file(mut self, relative: impl Into<String>, contents: impl Into<String>) -> Self {
        self.setup.seed_files.push(SeedFile {
            relative: relative.into(),
            contents: contents.into(),
        });
        self
    }

    pub fn assert_exit(self, code: i32) -> Self {
        self.with_assertion(Assertion::exit(code))
    }

    pub fn assert_count(self, expected: usize) -> Self {
        self.with_assertion(Assertion::count(expected))
    }

    pub fn assert_count_some(self) -> Self {
        self.with_assertion(Assertion::count_some())
    }

    pub fn assert_count_none(self) -> Self {
        self.with_assertion(Assertion::count_none())
    }

    pub fn assert_stdout(self, expected: impl Into<String>) -> Self {
        self.with_assertion(Assertion::stdout(expected))
    }

    pub fn assert_stdout_snapshot(self, snapshot: impl Into<String>) -> Self {
        self.with_assertion(Assertion::stdout_snapshot(snapshot))
    }

    pub fn assert_stderr(self, expected: impl Into<String>) -> Self {
        self.with_assertion(Assertion::stderr(expected))
    }

    pub fn assert_stderr_snapshot(self, snapshot: impl Into<String>) -> Self {
        self.with_assertion(Assertion::stderr_snapshot(snapshot))
    }

    pub fn assert_combined(self, expected: impl Into<String>) -> Self {
        self.with_assertion(Assertion::combined(expected))
    }

    pub fn assert_combined_snapshot(self, snapshot: impl Into<String>) -> Self {
        self.with_assertion(Assertion::combined_snapshot(snapshot))
    }

    pub fn assert_stdout_contains(self, needle: impl Into<String>) -> Self {
        self.with_assertion(Assertion::stdout_contains(needle))
    }

    pub fn assert_stderr_contains(self, needle: impl Into<String>) -> Self {
        self.with_assertion(Assertion::stderr_contains(needle))
    }

    pub fn assert_combined_contains(self, needle: impl Into<String>) -> Self {
        self.with_assertion(Assertion::combined_contains(needle))
    }

    pub fn assert_file_eq(self, relative: impl Into<String>, expected: impl Into<String>) -> Self {
        self.with_assertion(Assertion::file_eq(relative, expected))
    }

    pub fn assert_file_snapshot(
        self,
        relative: impl Into<String>,
        snapshot: impl Into<String>,
    ) -> Self {
        self.with_assertion(Assertion::file_snapshot(relative, snapshot))
    }

    pub fn assert_file_contains(
        self,
        relative: impl Into<String>,
        needle: impl Into<String>,
    ) -> Self {
        self.with_assertion(Assertion::file_contains(relative, needle))
    }

    pub fn capture(self) -> RunResult {
        self.execute(false)
    }

    pub fn run(self) -> RunResult {
        self.execute(true)
    }

    fn execute(self, assert_expectations: bool) -> RunResult {
        let TestCase {
            command,
            assertions,
            setup,
        } = self;

        let context = prepare_execution(&setup);
        let mut result = run_invocation(&command, &context.cwd);
        normalize_result(&mut result, &context, &setup);

        if assert_expectations {
            assert_default_exit(&assertions, result.status);
            for assertion in &assertions {
                evaluate_assertion(assertion, &command, &context, &result);
            }
        }

        result._temp_dir = context.temp_dir;
        result
    }
}

pub fn command(args: impl IntoIterator<Item = impl Into<String>>) -> TestCase {
    TestCase::new(TractorInvocation::new(args))
}

pub fn query_command(file: impl Into<String>, xpath: impl Into<String>) -> TestCase {
    let file = file.into();
    let xpath = xpath.into();
    TestCase::new(
        TractorInvocation::new(["query"])
            .arg(file)
            .arg("-x")
            .arg(xpath),
    )
}

pub fn inline_command(
    lang: impl Into<String>,
    source: impl Into<String>,
    xpath: impl Into<String>,
) -> TestCase {
    let lang = lang.into();
    let source = source.into();
    let xpath = xpath.into();
    TestCase::new(
        TractorInvocation::new(["query"])
            .arg("-s")
            .arg(source)
            .arg("-l")
            .arg(lang)
            .arg("-x")
            .arg(xpath),
    )
}

fn prepare_execution(setup: &HarnessSetup) -> ExecutionContext {
    let fixture = setup.fixture.clone();
    let fixture_root = fixture.as_deref().map(fixture_dir);
    let (cwd, temp_dir) = match (setup.temp_fixture, fixture_root.as_ref()) {
        (true, Some(source)) => {
            let temp_dir = TempDir::new().expect("failed to create temp fixture directory");
            if source.exists() {
                copy_dir_all(source, temp_dir.path());
            }
            (temp_dir.path().to_path_buf(), Some(temp_dir))
        }
        (true, None) => {
            let temp_dir = TempDir::new().expect("failed to create temp fixture directory");
            (temp_dir.path().to_path_buf(), Some(temp_dir))
        }
        (false, Some(source)) if source.exists() => (source.to_path_buf(), None),
        // Some suites are logically grouped under empty fixture directories.
        // Git does not preserve empty directories, so CI checkouts may not
        // contain them. Fall back to an empty temp workspace so command-based
        // tests still have a stable cwd without requiring placeholder files.
        (false, Some(_)) => {
            let temp_dir = TempDir::new().expect("failed to create temp fixture directory");
            (temp_dir.path().to_path_buf(), Some(temp_dir))
        }
        (false, None) => (repo_root(), None),
    };

    for file in &setup.seed_files {
        let path = cwd.join(&file.relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create seed file parent directory");
        }
        fs::write(&path, &file.contents).expect("failed to write seed file");
    }

    ExecutionContext {
        cwd,
        fixture,
        temp_dir,
    }
}

fn run_invocation(invocation: &TractorInvocation, cwd: &Path) -> RunResult {
    let mut command = Command::new(binary_path());
    command.current_dir(cwd);
    command.stdin(if invocation.stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    for arg in &invocation.args {
        command.arg(resolve_arg(arg, cwd));
    }
    if invocation.no_color {
        command.arg("--no-color");
    }

    let mut child = command.spawn().expect("failed to launch tractor binary");
    if let Some(input) = &invocation.stdin {
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

    RunResult {
        status: output.status.code().unwrap_or(-1),
        combined: combine_streams(&stdout, &stderr),
        stdout,
        stderr,
        cwd: cwd.to_path_buf(),
        _temp_dir: None,
    }
}

fn normalize_result(result: &mut RunResult, context: &ExecutionContext, setup: &HarnessSetup) {
    if let Some(replacement) = &setup.temp_prefix_replacement {
        result.stdout = replace_path_prefix(&result.stdout, &result.cwd, replacement);
        result.stderr = replace_path_prefix(&result.stderr, &result.cwd, replacement);
        result.combined = replace_path_prefix(&result.combined, &result.cwd, replacement);
    }

    if let Some(replacement) = &setup.fixture_prefix_replacement {
        if let Some(fixture) = &context.fixture {
            let root = fixture_dir(fixture);
            result.stdout = replace_path_prefix(&result.stdout, &root, replacement);
            result.stderr = replace_path_prefix(&result.stderr, &root, replacement);
            result.combined = replace_path_prefix(&result.combined, &root, replacement);
        }
    }

    for (from, to) in &setup.replacements {
        result.stdout = result.stdout.replace(from, to);
        result.stderr = result.stderr.replace(from, to);
        result.combined = result.combined.replace(from, to);
    }
}

fn assert_default_exit(assertions: &[Assertion], actual: i32) {
    if assertions
        .iter()
        .any(|assertion| matches!(assertion, Assertion::Exit(_)))
    {
        return;
    }
    assert_eq!(0, actual, "expected success exit code");
}

fn evaluate_assertion(
    assertion: &Assertion,
    command: &TractorInvocation,
    context: &ExecutionContext,
    result: &RunResult,
) {
    match assertion {
        Assertion::Exit(expected) => {
            assert_eq!(*expected, result.status, "unexpected exit code");
        }
        Assertion::Count(expected) => {
            let actual = observe_match_count(command, &context.cwd);
            match expected {
                CountExpectation::Exact(expected) => {
                    assert_eq!(*expected, actual, "unexpected match count");
                }
                CountExpectation::Some => {
                    assert!(actual > 0, "expected at least one match");
                }
                CountExpectation::None => {
                    assert_eq!(0, actual, "expected no matches");
                }
            }
        }
        Assertion::Stdout(expected) => {
            assert_eq!(
                load_expected(expected),
                result.stdout,
                "unexpected stdout output"
            );
        }
        Assertion::Stderr(expected) => {
            assert_eq!(
                load_expected(expected),
                result.stderr,
                "unexpected stderr output"
            );
        }
        Assertion::Combined(expected) => {
            assert_eq!(
                load_expected(expected),
                result.combined,
                "unexpected combined output"
            );
        }
        Assertion::StdoutContains(needle) => {
            assert!(
                result.stdout.contains(needle),
                "expected stdout to contain {needle:?}, got:\n{}",
                result.stdout
            );
        }
        Assertion::StderrContains(needle) => {
            assert!(
                result.stderr.contains(needle),
                "expected stderr to contain {needle:?}, got:\n{}",
                result.stderr
            );
        }
        Assertion::CombinedContains(needle) => {
            assert!(
                result.combined.contains(needle),
                "expected combined output to contain {needle:?}, got:\n{}",
                result.combined
            );
        }
        Assertion::FileEq { relative, expected } => {
            let actual = normalize_text(
                &fs::read_to_string(result.cwd.join(relative))
                    .unwrap_or_else(|_| panic!("failed to read {relative}")),
            );
            assert_eq!(
                load_expected(expected),
                actual,
                "unexpected file contents for {relative}"
            );
        }
        Assertion::FileContains { relative, needle } => {
            let actual = normalize_text(
                &fs::read_to_string(result.cwd.join(relative))
                    .unwrap_or_else(|_| panic!("failed to read {relative}")),
            );
            assert!(
                actual.contains(needle),
                "expected {relative} to contain {needle:?}, got:\n{actual}"
            );
        }
    }
}

fn observe_match_count(command: &TractorInvocation, cwd: &Path) -> usize {
    let subcommand = command
        .args
        .first()
        .and_then(|arg| match arg {
            CommandArg::Literal(value) => Some(value.as_str()),
            CommandArg::FixturePath { .. } => None,
        })
        .unwrap_or("query");

    match subcommand {
        "query" | "check" | "test" => {}
        _ => panic!("count assertions are only supported for query/check/test commands"),
    }

    let observed = run_invocation(&command.with_count_view(), cwd);
    observed
        .stdout
        .parse::<usize>()
        .unwrap_or_else(|e| panic!("failed to parse count output {:?}: {e}", observed.stdout))
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

fn resolve_arg(arg: &CommandArg, cwd: &Path) -> OsString {
    match arg {
        CommandArg::Literal(value) => value.into(),
        CommandArg::FixturePath { relative, absolute } => {
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
                .unwrap_or_else(|_| panic!("failed to read snapshot {path}")),
        ),
    }
}

fn normalize_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_string()
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
    let mut variants = Vec::new();

    for raw in candidate_path_strings(path) {
        push_variant(&mut variants, raw.clone());
        push_variant(&mut variants, raw.replace('\\', "/"));
    }

    variants
}

fn candidate_path_strings(path: &Path) -> Vec<String> {
    let raw = path.to_string_lossy().to_string();
    let mut candidates = vec![raw.clone()];

    #[allow(clippy::disallowed_methods)] // test helper: generate fallback path variants
    if let Ok(canonical) = fs::canonicalize(path) {
        let canonical = canonical.to_string_lossy().to_string();
        if canonical != raw {
            candidates.push(canonical);
        }
    }

    if let Some(file_name) = path.file_name() {
        let temp_joined = std::env::temp_dir().join(file_name);
        let temp_joined = temp_joined.to_string_lossy().to_string();
        if !candidates.contains(&temp_joined) {
            candidates.push(temp_joined);
        }
    }

    let mut expanded = Vec::new();
    for candidate in candidates {
        expanded.push(candidate.clone());
        if let Some(stripped) = strip_windows_verbatim_prefix(&candidate) {
            expanded.push(stripped);
        }
    }

    expanded
}

fn strip_windows_verbatim_prefix(path: &str) -> Option<String> {
    path.strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| path.strip_prefix(r"\\?\").map(str::to_string))
        .or_else(|| path.strip_prefix("//?/UNC/").map(|rest| format!("//{rest}")))
        .or_else(|| path.strip_prefix("//?/").map(str::to_string))
}

fn push_variant(variants: &mut Vec<String>, value: String) {
    if !variants.contains(&value) {
        variants.push(value);
    }
}

#[cfg(test)]
mod support_tests {
    use super::replace_path_prefix;
    use std::path::{Path, PathBuf};

    #[test]
    fn replace_path_prefix_matches_windows_verbatim_paths() {
        let path = Path::new(r"\\?\C:\Users\runneradmin\AppData\Local\Temp\.tmp123");
        let output = "C:/Users/runneradmin/AppData/Local/Temp/.tmp123/app-config.json:3:13: note";

        assert_eq!(
            "app-config.json:3:13: note",
            replace_path_prefix(output, path, "")
        );
    }

    #[test]
    fn replace_path_prefix_matches_plain_temp_dir_joined_from_basename() {
        let temp_path = std::env::temp_dir().join(".tmp123");
        let output = format!(
            "{}/app-config.json:3:13: note",
            temp_path.to_string_lossy().replace('\\', "/")
        );
        let aliased_path = if cfg!(windows) {
            PathBuf::from(r"X:\different\spelling\.tmp123")
        } else {
            PathBuf::from("/different/spelling/.tmp123")
        };

        assert_eq!(
            "app-config.json:3:13: note",
            replace_path_prefix(&output, &aliased_path, "")
        );
    }
}

fn copy_dir_all(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("failed to create destination directory");

    for entry in fs::read_dir(source).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let file_type = entry
            .file_type()
            .expect("failed to inspect directory entry");
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &destination_path);
        } else {
            fs::copy(entry.path(), destination_path).expect("failed to copy fixture file");
        }
    }
}

fn parse_dsl_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }

        if chars[index] == '"' {
            index += 1;
            let mut value = String::new();
            while index < chars.len() {
                match chars[index] {
                    '"' => {
                        index += 1;
                        break;
                    }
                    '\\' => {
                        index += 1;
                        let escaped = chars.get(index).copied().unwrap_or_else(|| {
                            panic!("unterminated escape in DSL string: {input}")
                        });
                        value.push(match escaped {
                            '\\' => '\\',
                            '"' => '"',
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '0' => '\0',
                            other => panic!("unsupported escape \\{other} in DSL string: {input}"),
                        });
                        index += 1;
                    }
                    ch => {
                        value.push(ch);
                        index += 1;
                    }
                }
            }
            words.push(value);
            continue;
        }

        let start = index;
        while index < chars.len() && !chars[index].is_whitespace() {
            index += 1;
        }
        words.push(chars[start..index].iter().collect());
    }

    words
}

// ---------------------------------------------------------------------------
// Layer 1: suite structure and lowering macros
// ---------------------------------------------------------------------------

#[doc(hidden)]
#[macro_export]
macro_rules! tractor_long_flag {
    ($name:ident) => {
        concat!("--", stringify!($name))
    };
    ($first:ident, $second:ident) => {
        concat!("--", stringify!($first), "-", stringify!($second))
    };
    ($first:ident, $second:ident, $third:ident) => {
        concat!(
            "--",
            stringify!($first),
            "-",
            stringify!($second),
            "-",
            stringify!($third)
        )
    };
}

/// Capture a CLI-shaped Tractor command into a `TractorInvocation`.
#[doc(hidden)]
#[macro_export]
macro_rules! tractor_invocation {
    ($($command:tt)+) => {
        $crate::support::TractorInvocation::from_dsl(::core::stringify!($($command)+))
    };
}

/// Parse one or more harness assertions.
#[doc(hidden)]
#[macro_export]
macro_rules! cli_assertions {
    ({ $($assertions:tt)* }) => {{
        let mut assertions = ::std::vec::Vec::new();
        $crate::cli_assertions!(@push assertions; $($assertions)*);
        assertions
    }};
    (@push $assertions:ident;) => {};
    (@push $assertions:ident; exit $code:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::exit($code));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; count $count:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::count($count));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; count some; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::count_some());
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; count none; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::count_none());
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; stdout $expected:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::stdout($expected));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; stdout_snapshot $snapshot:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::stdout_snapshot($snapshot));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; stderr $expected:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::stderr($expected));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; stderr_snapshot $snapshot:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::stderr_snapshot($snapshot));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; combined $expected:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::combined($expected));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; combined_snapshot $snapshot:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::combined_snapshot($snapshot));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; stdout_contains $needle:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::stdout_contains($needle));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; stderr_contains $needle:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::stderr_contains($needle));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; combined_contains $needle:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::combined_contains($needle));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; file_eq $path:literal $expected:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::file_eq($path, $expected));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; file_snapshot $path:literal $snapshot:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::file_snapshot($path, $snapshot));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
    (@push $assertions:ident; file_contains $path:literal $needle:literal; $($rest:tt)*) => {{
        $assertions.push($crate::support::Assertion::file_contains($path, $needle));
        $crate::cli_assertions!(@push $assertions; $($rest)*);
    }};
}

/// Lower a DSL case into `TestCase { command, assertions }`.
#[doc(hidden)]
#[macro_export]
macro_rules! cli_case {
    (tractor $($rest:tt)*) => {
        $crate::cli_case!(@one_liner_command [] $($rest)*)
    };
    (@one_liner_command [$($command:tt)*] => $assertion:ident $($rest:tt)*) => {
        $crate::cli_case!(@one_liner_assert [$($command)*] $assertion [] $($rest)*)
    };
    (@one_liner_command [$($command:tt)*] $next:tt $($rest:tt)*) => {
        $crate::cli_case!(@one_liner_command [$($command)* $next] $($rest)*)
    };
    (@one_liner_assert [$($command:tt)*] $assertion:ident [$($args:tt)*]) => {
        $crate::cli_case!({
            tractor $($command)*;
            expect => { $assertion $($args)*; }
        })
    };
    (@one_liner_assert [$($command:tt)*] $assertion:ident [$($args:tt)*] $next:tt $($rest:tt)*) => {
        $crate::cli_case!(@one_liner_assert [$($command)*] $assertion [$($args)* $next] $($rest)*)
    };
    ({
        tractor $($rest:tt)*
    }) => {
        $crate::cli_case!(@block_command [] $($rest)*)
    };
    (@block_command [$($command:tt)*] ; expect => { $($assertions:tt)* } ) => {{
        $crate::support::TestCase::new($crate::tractor_invocation!($($command)+))
            .with_assertions($crate::cli_assertions!({ $($assertions)* }))
    }};
    (@block_command [$($command:tt)*] ; expect => $assertion:ident $($rest:tt)*) => {
        $crate::cli_case!(@block_assert [$($command)*] $assertion [] $($rest)*)
    };
    (@block_command [$($command:tt)*] ;) => {{
        $crate::support::TestCase::new($crate::tractor_invocation!($($command)+))
    }};
    (@block_command [$($command:tt)*] $next:tt $($rest:tt)*) => {
        $crate::cli_case!(@block_command [$($command)* $next] $($rest)*)
    };
    (@block_assert [$($command:tt)*] $assertion:ident [$($args:tt)*] ; ) => {
        $crate::cli_case!({
            tractor $($command)*;
            expect => { $assertion $($args)*; }
        })
    };
    (@block_assert [$($command:tt)*] $assertion:ident [$($args:tt)*] $next:tt $($rest:tt)*) => {
        $crate::cli_case!(@block_assert [$($command)*] $assertion [$($args)* $next] $($rest)*)
    };
}

/// Define a fixture-backed module of CLI tests.
macro_rules! cli_suite {
    ($module:ident in $fixture:literal { $($cases:tt)* }) => {
        mod $module {
            cli_suite!(@cases $fixture; $($cases)*);
        }
    };
    (@cases $fixture:literal;) => {};
    (@cases $fixture:literal; ; $($rest:tt)*) => {
        cli_suite!(@cases $fixture; $($rest)*);
    };
    (@cases $fixture:literal; $name:ident => tractor $($rest:tt)*) => {
        cli_suite!(@one_liner_command $fixture $name [] $($rest)*);
    };
    (@cases $fixture:literal; $name:ident => $body:block $($rest:tt)*) => {
        #[test]
        fn $name() {
            $crate::cli_case!($body)
                .in_fixture($fixture)
                .run();
        }

        cli_suite!(@cases $fixture; $($rest)*);
    };
    (@one_liner_command $fixture:literal $name:ident [$($command:tt)*] => $assertion:ident $($rest:tt)*) => {
        cli_suite!(@one_liner_assert $fixture $name [$($command)*] $assertion [] $($rest)*);
    };
    (@one_liner_command $fixture:literal $name:ident [$($command:tt)*] $next:tt $($rest:tt)*) => {
        cli_suite!(@one_liner_command $fixture $name [$($command)* $next] $($rest)*);
    };
    (@one_liner_assert $fixture:literal $name:ident [$($command:tt)*] $assertion:ident [$($args:tt)*] ; $($rest:tt)*) => {
        #[test]
        fn $name() {
            $crate::cli_case!(tractor $($command)* => $assertion $($args)*)
                .in_fixture($fixture)
                .run();
        }

        cli_suite!(@cases $fixture; $($rest)*);
    };
    (@one_liner_assert $fixture:literal $name:ident [$($command:tt)*] $assertion:ident [$($args:tt)*] $next:tt $($rest:tt)*) => {
        cli_suite!(@one_liner_assert $fixture $name [$($command)*] $assertion [$($args)* $next] $($rest)*);
    };
}
