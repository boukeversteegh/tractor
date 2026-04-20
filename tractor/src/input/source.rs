//! Unified source abstraction: virtual paths as first-class citizens.
//!
//! A [`Source`] carries everything the executor needs to process one input:
//! a normalized path, a language, and a way to fetch its bytes. Whether
//! those bytes live on disk or in memory is a property of the source, not
//! a structural split at the operation level.
//!
//! This replaces the former `files: Vec<String>` + `inline_source: Option<String>`
//! + `language: Option<String>` trio on each `Operation`, and removes the
//! Phase-2-inline vs Phase-3-files branch that used to live in every executor.
//!
//! Design: see `docs/design-unified-source.md`.
//!
//! Factor separation:
//!   - Content provenance  → `SourceContent` variant
//!   - Match identity      → `Source.path` (used by globs, diff-lines, reports)
//!   - Display identity    → `Source.path` (same; virtual paths render as themselves)
//!
//! The sentinel `PATHLESS_LABEL` only appears when the user piped inline
//! content *without* a positional path — the one genuinely path-less case.

use std::borrow::Cow;
use std::io;
use std::path::Path;
use std::sync::Arc;

use tractor::tree_mode::TreeMode;
use tractor::{
    parse_string_to_documents_with_options, parse_to_documents, NormalizedPath, XeeParseResult,
};
use tractor::parser::ParseError;

// The sentinel for path-less sources lives in the library crate; re-export
// here so existing `crate::input::PATHLESS_LABEL` callers still work.
pub use tractor::PATHLESS_LABEL;

/// A single input to an operation. Virtual and disk sources share this
/// shape so the executor can treat them uniformly.
#[derive(Debug, Clone)]
pub struct Source {
    /// Absolute, normalized path. For virtual sources this is the user's
    /// `--stdin-filename`-equivalent; for disk sources it's the resolved
    /// file path. For a path-less inline source it's the sentinel label
    /// wrapped as a `NormalizedPath` so downstream code doesn't special-case.
    pub path: NormalizedPath,

    /// Resolved language (from `-l` override or `detect_language(path)`).
    /// Resolved once at the input boundary — downstream never re-detects.
    pub language: String,

    /// How to obtain the bytes.
    pub content: SourceContent,
}

#[derive(Debug, Clone)]
pub enum SourceContent {
    /// Read lazily from disk at parse time. Preserves today's behaviour
    /// for large file sets: no bytes materialize until the parallel worker
    /// actually parses the source.
    Disk,
    /// Content is already in memory (piped stdin or `-s/--string`).
    /// Wrapped in `Arc` so it can be cheaply cloned across workers and
    /// shared with the diff-builder without re-reading.
    Inline(Arc<String>),
}

/// The three-way domain state of a source, combining content provenance
/// (disk vs. in-memory) with path identity (real/virtual path vs. the
/// pathless sentinel).
///
/// This is the primary classifier; `Source::is_virtual` and
/// `Source::is_pathless` are thin derivations of this enum so existing
/// single-question boolean callers stay ergonomic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDisposition {
    /// On-disk file. Reads lazily from the filesystem; writes go back to
    /// disk.
    Disk,
    /// In-memory content carrying a user-supplied virtual path (e.g. via
    /// `--stdin-filename`). Writes are captured; display keeps the path.
    InlineWithPath,
    /// In-memory content with no user-supplied path — uses the
    /// `PATHLESS_LABEL` sentinel. Writes are captured; display omits the
    /// path prefix.
    InlinePathless,
}

impl Source {
    /// Construct a disk source. Caller is expected to have already resolved
    /// the language (via override or `detect_language`).
    pub fn disk(path: NormalizedPath, language: impl Into<String>) -> Self {
        Self {
            path,
            language: language.into(),
            content: SourceContent::Disk,
        }
    }

    /// Construct an inline source at the given virtual path.
    pub fn inline_at(
        path: NormalizedPath,
        language: impl Into<String>,
        content: impl Into<Arc<String>>,
    ) -> Self {
        Self {
            path,
            language: language.into(),
            content: SourceContent::Inline(content.into()),
        }
    }

    /// Construct a path-less inline source (user piped content without a
    /// positional path). Gets the sentinel label so downstream sees a path
    /// like everything else.
    pub fn inline_pathless(
        language: impl Into<String>,
        content: impl Into<Arc<String>>,
    ) -> Self {
        Self {
            path: NormalizedPath::new(PATHLESS_LABEL),
            language: language.into(),
            content: SourceContent::Inline(content.into()),
        }
    }

    /// The source's domain state as a three-variant enum. This is the
    /// primary classifier — prefer `match source.disposition()` over
    /// stacking `is_virtual()` + `is_pathless()` boolean checks when the
    /// call site actually cares about which of the three states applies.
    pub fn disposition(&self) -> SourceDisposition {
        match &self.content {
            SourceContent::Disk => SourceDisposition::Disk,
            SourceContent::Inline(_) => {
                if self.path.as_str() == PATHLESS_LABEL {
                    SourceDisposition::InlinePathless
                } else {
                    SourceDisposition::InlineWithPath
                }
            }
        }
    }

    /// True for inline sources (both pathless and with a virtual path).
    /// The one place this matters outside the parse site is the mutation
    /// layer, which refuses to write inline content back to disk.
    ///
    /// Thin derivation of [`Source::disposition`].
    pub fn is_virtual(&self) -> bool {
        !matches!(self.disposition(), SourceDisposition::Disk)
    }

    /// True for a path-less inline source. Used by the formatter to
    /// suppress location prefixes in the one narrow case where there's no
    /// meaningful path to display.
    ///
    /// Thin derivation of [`Source::disposition`].
    pub fn is_pathless(&self) -> bool {
        matches!(self.disposition(), SourceDisposition::InlinePathless)
    }

    /// Fetch the bytes to parse. Borrows from memory for inline sources;
    /// reads from disk for `SourceContent::Disk`. Called inside the
    /// parallel worker so laziness is preserved for the file flow.
    pub fn read(&self) -> io::Result<Cow<'_, str>> {
        match &self.content {
            SourceContent::Disk => {
                std::fs::read_to_string(Path::new(self.path.as_str())).map(Cow::Owned)
            }
            SourceContent::Inline(s) => Ok(Cow::Borrowed(s.as_str())),
        }
    }

    /// The path as a `&str`, for places that still accept a bare path label
    /// (e.g. `parse_string_to_documents`, diagnostic `ReportMatch.file`).
    pub fn path_str(&self) -> &str {
        self.path.as_str()
    }

    /// For inline sources, the content as a `&str`. `None` for disk sources.
    pub fn inline_content(&self) -> Option<&str> {
        match &self.content {
            SourceContent::Inline(s) => Some(s.as_str()),
            SourceContent::Disk => None,
        }
    }

    /// Parse this source into queryable `Documents`.
    ///
    /// Dispatches on content kind:
    /// - `Inline` → in-memory parse via `parse_string_to_documents_with_options`
    /// - `Disk`   → file parse via `parse_to_documents` (preserves the existing
    ///              disk-read-then-parse flow, including ambiguous-extension
    ///              checks when the language was auto-detected)
    ///
    /// `lang_override` lets the caller (e.g. `run_rules`) substitute a
    /// rule-level language override for the source's default language.
    pub fn parse(
        &self,
        lang_override: Option<&str>,
        tree_mode: Option<TreeMode>,
        ignore_whitespace: bool,
        parse_depth: Option<usize>,
    ) -> Result<XeeParseResult, ParseError> {
        let lang = lang_override.unwrap_or(&self.language);
        match &self.content {
            SourceContent::Disk => parse_to_documents(
                Path::new(self.path.as_str()),
                Some(lang),
                tree_mode,
                ignore_whitespace,
                parse_depth,
            ),
            SourceContent::Inline(content) => parse_string_to_documents_with_options(
                content.as_str(),
                lang,
                self.path.as_str().to_string(),
                tree_mode,
                ignore_whitespace,
                parse_depth,
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathless_has_sentinel_label() {
        let s = Source::inline_pathless("csharp", Arc::new("code".to_string()));
        assert!(s.is_virtual());
        assert!(s.is_pathless());
        assert_eq!(s.path_str(), PATHLESS_LABEL);
        assert_eq!(s.inline_content(), Some("code"));
    }

    #[test]
    fn inline_at_preserves_path() {
        let s = Source::inline_at(
            NormalizedPath::new("src/Foo.cs"),
            "csharp",
            Arc::new("code".to_string()),
        );
        assert!(s.is_virtual());
        assert!(!s.is_pathless());
        assert_eq!(s.path_str(), "src/Foo.cs");
    }

    #[test]
    fn disk_is_not_virtual() {
        let s = Source::disk(NormalizedPath::new("src/foo.rs"), "rust");
        assert!(!s.is_virtual());
        assert!(!s.is_pathless());
        assert!(s.inline_content().is_none());
    }

    #[test]
    fn disposition_covers_all_three_states() {
        // Disk source.
        let disk = Source::disk(NormalizedPath::new("src/foo.rs"), "rust");
        assert_eq!(disk.disposition(), SourceDisposition::Disk);

        // Inline source with a user-supplied virtual path.
        let with_path = Source::inline_at(
            NormalizedPath::new("src/Foo.cs"),
            "csharp",
            Arc::new("code".to_string()),
        );
        assert_eq!(with_path.disposition(), SourceDisposition::InlineWithPath);

        // Path-less inline source (PATHLESS_LABEL sentinel).
        let pathless = Source::inline_pathless("csharp", Arc::new("code".to_string()));
        assert_eq!(pathless.disposition(), SourceDisposition::InlinePathless);

        // And the boolean derivations must stay consistent with the enum.
        assert!(!disk.is_virtual());
        assert!(!disk.is_pathless());
        assert!(with_path.is_virtual());
        assert!(!with_path.is_pathless());
        assert!(pathless.is_virtual());
        assert!(pathless.is_pathless());
    }

    #[test]
    fn inline_read_borrows() {
        let s = Source::inline_at(
            NormalizedPath::new("virt.txt"),
            "text",
            Arc::new("hello".to_string()),
        );
        let cow = s.read().unwrap();
        assert_eq!(cow.as_ref(), "hello");
        assert!(matches!(cow, Cow::Borrowed(_)));
    }
}
