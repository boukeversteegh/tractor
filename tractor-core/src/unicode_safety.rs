//! Unicode safety scanner for detecting invisible/malicious characters in source code.
//!
//! Detects attack vectors including:
//! - **Trojan Source** (CVE-2021-42574): Bidirectional control characters that reorder
//!   displayed code while preserving malicious execution order.
//! - **GlassWorm**: Invisible Unicode characters (variation selectors, PUA, tags) used
//!   to encode hidden payloads that are decoded and executed at runtime.
//! - **Homoglyph attacks** (CVE-2021-42694): Characters that look identical to ASCII
//!   but have different Unicode codepoints, enabling invisible identifier substitution.
//! - **Zero-width injections**: Characters with no visual rendering used to smuggle
//!   data or alter string behavior without visible change.

use std::fmt;

// ---------------------------------------------------------------------------
// Threat categories
// ---------------------------------------------------------------------------

/// Category of Unicode threat detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatCategory {
    /// Bidirectional control characters (Trojan Source attack)
    BidiControl,
    /// Zero-width characters (invisible content injection)
    ZeroWidth,
    /// Variation selectors abused by GlassWorm to encode hidden payloads
    VariationSelector,
    /// Unicode tag characters (U+E0001–U+E007F) used for invisible data
    TagCharacter,
    /// Supplementary Private Use Area characters
    SupplementaryPrivateUse,
    /// Uncommon invisible formatting characters
    InvisibleFormatting,
    /// Homoglyph: character visually similar to common ASCII
    Homoglyph,
    /// Double BOM at file start
    DoubleBom,
}

impl ThreatCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreatCategory::BidiControl => "bidi-control",
            ThreatCategory::ZeroWidth => "zero-width",
            ThreatCategory::VariationSelector => "variation-selector",
            ThreatCategory::TagCharacter => "tag-character",
            ThreatCategory::SupplementaryPrivateUse => "supplementary-private-use",
            ThreatCategory::InvisibleFormatting => "invisible-formatting",
            ThreatCategory::Homoglyph => "homoglyph",
            ThreatCategory::DoubleBom => "double-bom",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ThreatCategory::BidiControl => "Bidirectional control character can reorder displayed code (Trojan Source)",
            ThreatCategory::ZeroWidth => "Zero-width character can hide invisible content",
            ThreatCategory::VariationSelector => "Variation selector can encode hidden payloads (GlassWorm)",
            ThreatCategory::TagCharacter => "Tag character can carry invisible data",
            ThreatCategory::SupplementaryPrivateUse => "Supplementary Private Use Area character (GlassWorm payload carrier)",
            ThreatCategory::InvisibleFormatting => "Invisible formatting character has no visible rendering",
            ThreatCategory::Homoglyph => "Character visually resembles ASCII but has a different codepoint",
            ThreatCategory::DoubleBom => "File starts with a double BOM (byte order mark)",
        }
    }

    pub fn severity(&self) -> &'static str {
        match self {
            ThreatCategory::BidiControl => "error",
            ThreatCategory::ZeroWidth => "warning",
            ThreatCategory::VariationSelector => "error",
            ThreatCategory::TagCharacter => "error",
            ThreatCategory::SupplementaryPrivateUse => "error",
            ThreatCategory::InvisibleFormatting => "warning",
            ThreatCategory::Homoglyph => "warning",
            ThreatCategory::DoubleBom => "warning",
        }
    }
}

impl fmt::Display for ThreatCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Finding
// ---------------------------------------------------------------------------

/// A single suspicious Unicode character found in a file.
#[derive(Debug, Clone)]
pub struct UnicodeFinding {
    pub line: u32,
    pub column: u32,
    pub codepoint: char,
    pub category: ThreatCategory,
}

impl UnicodeFinding {
    pub fn reason(&self) -> String {
        format!(
            "{} (U+{:04X}, {})",
            self.category.description(),
            self.codepoint as u32,
            self.category.as_str(),
        )
    }
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify a character as a potential Unicode threat.
/// Returns None for safe characters.
pub fn classify_char(ch: char) -> Option<ThreatCategory> {
    let cp = ch as u32;

    // 1. Bidirectional control characters (Trojan Source - CVE-2021-42574)
    //    These reorder how text is displayed without changing execution order.
    match cp {
        0x200E          // LEFT-TO-RIGHT MARK
        | 0x200F        // RIGHT-TO-LEFT MARK
        | 0x202A        // LEFT-TO-RIGHT EMBEDDING
        | 0x202B        // RIGHT-TO-LEFT EMBEDDING
        | 0x202C        // POP DIRECTIONAL FORMATTING
        | 0x202D        // LEFT-TO-RIGHT OVERRIDE
        | 0x202E        // RIGHT-TO-LEFT OVERRIDE
        | 0x2066        // LEFT-TO-RIGHT ISOLATE
        | 0x2067        // RIGHT-TO-LEFT ISOLATE
        | 0x2068        // FIRST STRONG ISOLATE
        | 0x2069        // POP DIRECTIONAL ISOLATE
        => return Some(ThreatCategory::BidiControl),
        _ => {}
    }

    // 2. Zero-width characters (invisible content injection)
    match cp {
        0x200B          // ZERO WIDTH SPACE
        | 0x200C        // ZERO WIDTH NON-JOINER
        | 0x200D        // ZERO WIDTH JOINER
        | 0xFEFF        // ZERO WIDTH NO-BREAK SPACE (BOM when not at file start)
        | 0x2060        // WORD JOINER
        | 0x2061        // FUNCTION APPLICATION
        | 0x2062        // INVISIBLE TIMES
        | 0x2063        // INVISIBLE SEPARATOR
        | 0x2064        // INVISIBLE PLUS
        | 0x00AD        // SOFT HYPHEN
        => return Some(ThreatCategory::ZeroWidth),
        _ => {}
    }

    // 3. Variation selectors (GlassWorm attack vector)
    //    VS1-VS16: U+FE00–U+FE0F
    //    VS17-VS256: U+E0100–U+E01EF
    if (0xFE00..=0xFE0F).contains(&cp) || (0xE0100..=0xE01EF).contains(&cp) {
        return Some(ThreatCategory::VariationSelector);
    }

    // 4. Unicode tag characters (invisible data encoding)
    //    U+E0001 (LANGUAGE TAG) and U+E0020–U+E007F (TAG SPACE through CANCEL TAG)
    if cp == 0xE0001 || (0xE0020..=0xE007F).contains(&cp) {
        return Some(ThreatCategory::TagCharacter);
    }

    // 5. Supplementary Private Use Areas (GlassWorm payload carriers)
    //    Plane 15: U+F0000–U+FFFFD
    //    Plane 16: U+100000–U+10FFFD
    if (0xF0000..=0xFFFFF).contains(&cp) || (0x100000..=0x10FFFF).contains(&cp) {
        return Some(ThreatCategory::SupplementaryPrivateUse);
    }

    // 6. Other invisible formatting characters
    match cp {
        0x00A0          // NO-BREAK SPACE (in source code context, suspicious)
        | 0x115F        // HANGUL CHOSEONG FILLER
        | 0x1160        // HANGUL JUNGSEONG FILLER
        | 0x3164        // HANGUL FILLER
        | 0xFFA0        // HALFWIDTH HANGUL FILLER
        | 0x180E        // MONGOLIAN VOWEL SEPARATOR
        | 0x2800        // BRAILLE PATTERN BLANK
        => return Some(ThreatCategory::InvisibleFormatting),
        _ => {}
    }

    // 7. Common homoglyphs (characters that look like ASCII but aren't)
    if let Some(_) = homoglyph_ascii_equivalent(ch) {
        return Some(ThreatCategory::Homoglyph);
    }

    None
}

/// Returns the ASCII character that a homoglyph visually resembles, if any.
fn homoglyph_ascii_equivalent(ch: char) -> Option<char> {
    // Only flag characters that are commonly used in identifiers/operators
    // and that are very hard to distinguish visually from ASCII.
    match ch {
        // Latin-like homoglyphs
        '\u{0410}' => Some('A'),    // Cyrillic А
        '\u{0412}' => Some('B'),    // Cyrillic В
        '\u{0421}' => Some('C'),    // Cyrillic С
        '\u{0415}' => Some('E'),    // Cyrillic Е
        '\u{041D}' => Some('H'),    // Cyrillic Н
        '\u{041A}' => Some('K'),    // Cyrillic К
        '\u{041C}' => Some('M'),    // Cyrillic М
        '\u{041E}' => Some('O'),    // Cyrillic О
        '\u{0420}' => Some('P'),    // Cyrillic Р
        '\u{0422}' => Some('T'),    // Cyrillic Т
        '\u{0425}' => Some('X'),    // Cyrillic Х
        '\u{0430}' => Some('a'),    // Cyrillic а
        '\u{0435}' => Some('e'),    // Cyrillic е
        '\u{043E}' => Some('o'),    // Cyrillic о
        '\u{0440}' => Some('p'),    // Cyrillic р
        '\u{0441}' => Some('c'),    // Cyrillic с
        '\u{0443}' => Some('y'),    // Cyrillic у
        '\u{0445}' => Some('x'),    // Cyrillic х
        '\u{0455}' => Some('s'),    // Cyrillic ѕ
        '\u{0456}' => Some('i'),    // Cyrillic і
        '\u{0458}' => Some('j'),    // Cyrillic ј
        '\u{04BB}' => Some('h'),    // Cyrillic һ
        '\u{0501}' => Some('d'),    // Cyrillic ԁ

        // Greek homoglyphs
        '\u{0391}' => Some('A'),    // Greek Α
        '\u{0392}' => Some('B'),    // Greek Β
        '\u{0395}' => Some('E'),    // Greek Ε
        '\u{0396}' => Some('Z'),    // Greek Ζ
        '\u{0397}' => Some('H'),    // Greek Η
        '\u{0399}' => Some('I'),    // Greek Ι
        '\u{039A}' => Some('K'),    // Greek Κ
        '\u{039C}' => Some('M'),    // Greek Μ
        '\u{039D}' => Some('N'),    // Greek Ν
        '\u{039F}' => Some('O'),    // Greek Ο
        '\u{03A1}' => Some('P'),    // Greek Ρ
        '\u{03A4}' => Some('T'),    // Greek Τ
        '\u{03A5}' => Some('Y'),    // Greek Υ
        '\u{03A7}' => Some('X'),    // Greek Χ
        '\u{03BF}' => Some('o'),    // Greek ο
        '\u{03B1}' => Some('a'),    // Greek α (less similar but used in attacks)

        // Fullwidth Latin (often invisible in monospace fonts)
        '\u{FF21}'..='\u{FF3A}' => Some((ch as u32 - 0xFF21 + b'A' as u32) as u8 as char),
        '\u{FF41}'..='\u{FF5A}' => Some((ch as u32 - 0xFF41 + b'a' as u32) as u8 as char),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

/// Scan file content for suspicious Unicode characters.
///
/// Returns a list of findings sorted by (line, column).
/// The `at_offset` parameter allows skipping the BOM at byte offset 0.
pub fn scan_content(content: &str) -> Vec<UnicodeFinding> {
    let mut findings = Vec::new();
    let mut line: u32 = 1;
    let mut column: u32 = 1;
    let mut char_offset: usize = 0;
    let mut had_bom = false;

    for ch in content.chars() {
        // Skip BOM at file start (char offset 0) — that's legitimate
        if ch == '\u{FEFF}' && char_offset == 0 {
            char_offset += 1;
            had_bom = true;
            continue;
        }

        // Double BOM: report as a distinct file-level finding at line 1, column 0
        if ch == '\u{FEFF}' && char_offset == 1 && had_bom {
            findings.push(UnicodeFinding {
                line: 1,
                column: 0,
                codepoint: ch,
                category: ThreatCategory::DoubleBom,
            });
            char_offset += 1;
            continue;
        }

        if let Some(category) = classify_char(ch) {
            findings.push(UnicodeFinding {
                line,
                column,
                codepoint: ch,
                category,
            });
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
        char_offset += 1;
    }

    findings
}

/// Check if a file appears to be binary (contains null bytes in first 8KB).
pub fn is_likely_binary(content: &[u8]) -> bool {
    let check_len = content.len().min(8192);
    content[..check_len].contains(&0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bidi_detection() {
        // LEFT-TO-RIGHT OVERRIDE
        let content = "let x = \u{202D}admin\u{202C};";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].category, ThreatCategory::BidiControl);
        assert_eq!(findings[0].codepoint, '\u{202D}');
        assert_eq!(findings[1].category, ThreatCategory::BidiControl);
        assert_eq!(findings[1].codepoint, '\u{202C}');
    }

    #[test]
    fn test_zero_width_detection() {
        let content = "let x\u{200B} = 1;";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, ThreatCategory::ZeroWidth);
        assert_eq!(findings[0].column, 6);
    }

    #[test]
    fn test_variation_selector_detection() {
        // GlassWorm-style payload using variation selectors
        let content = "var a = \"\u{FE00}\u{FE01}\u{FE02}\";";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 3);
        for f in &findings {
            assert_eq!(f.category, ThreatCategory::VariationSelector);
        }
    }

    #[test]
    fn test_supplementary_pua_detection() {
        let content = "// \u{F0000}";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, ThreatCategory::SupplementaryPrivateUse);
    }

    #[test]
    fn test_tag_character_detection() {
        let content = "x\u{E0001}\u{E0020}y";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].category, ThreatCategory::TagCharacter);
        assert_eq!(findings[1].category, ThreatCategory::TagCharacter);
    }

    #[test]
    fn test_homoglyph_detection() {
        // Cyrillic 'а' (U+0430) looks like Latin 'a'
        let content = "let \u{0430} = 1;";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, ThreatCategory::Homoglyph);
    }

    #[test]
    fn test_bom_at_start_is_allowed() {
        let content = "\u{FEFF}let x = 1;";
        let findings = scan_content(content);
        assert!(findings.is_empty(), "BOM at file start should be ignored");
    }

    #[test]
    fn test_bom_not_at_start_is_flagged() {
        let content = "let x = \u{FEFF}1;";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, ThreatCategory::ZeroWidth);
    }

    #[test]
    fn test_double_bom_detected() {
        let content = "\u{FEFF}\u{FEFF}let x = 1;";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, ThreatCategory::DoubleBom);
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[0].column, 0);
    }

    #[test]
    fn test_clean_code() {
        let content = "fn main() {\n    println!(\"Hello, world!\");\n}\n";
        let findings = scan_content(content);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_line_column_tracking() {
        let content = "line1\nline2\u{200B}end\nline3";
        let findings = scan_content(content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[0].column, 6);
    }

    #[test]
    fn test_binary_detection() {
        assert!(is_likely_binary(b"hello\x00world"));
        assert!(!is_likely_binary(b"hello world"));
    }

    #[test]
    fn test_multiple_categories_in_one_file() {
        let content = "let \u{0430} = \"\u{202D}test\u{202C}\u{FE00}\";";
        let findings = scan_content(content);
        let categories: Vec<_> = findings.iter().map(|f| f.category).collect();
        assert!(categories.contains(&ThreatCategory::Homoglyph));
        assert!(categories.contains(&ThreatCategory::BidiControl));
        assert!(categories.contains(&ThreatCategory::VariationSelector));
    }

    #[test]
    fn test_glassworm_marker() {
        // Simulate a GlassWorm-style encoded payload using variation selectors
        let payload = "eval(\"\u{FE00}\u{FE01}\u{FE02}\u{FE03}\u{FE04}\u{FE05}\u{FE06}\u{FE07}\u{FE08}\u{FE09}\u{FE0A}\u{FE0B}\u{FE0C}\u{FE0D}\u{FE0E}\u{FE0F}\")";
        let findings = scan_content(payload);
        assert_eq!(findings.len(), 16);
        for f in &findings {
            assert_eq!(f.category, ThreatCategory::VariationSelector);
        }
    }
}
