//! Input contract: decoding, limits, BOM handling, and the raw-source
//! guard. Fail-closed boundary.

use crate::{AnalysisError, Config, InputFormat};
use sha2::{Digest, Sha256};
use std::ops::Range;

#[derive(Debug, Clone)]
pub enum FormatData {
    Markdown,
    Text,
}

#[derive(Debug, Clone)]
pub struct Prepared {
    /// sha256 over the original bytes as received (pre BOM strip).
    pub sha256: String,
    pub original_len: usize,
    pub bom_stripped: bool,
    pub mixed_line_endings: bool,
    /// The post-BOM-strip payload every offset indexes.
    pub text: String,
    pub format: FormatData,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

pub fn prepare(input: &[u8], config: &Config) -> Result<Prepared, AnalysisError> {
    if input.len() > config.limits.max_bytes {
        return Err(AnalysisError::UnsupportedInput(format!(
            "input is {} bytes, over the {} byte limit",
            input.len(),
            config.limits.max_bytes
        )));
    }
    let sha256 = sha256_hex(input);
    let (payload, bom_stripped) = match input.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (rest, true),
        None => (input, false),
    };
    let text = std::str::from_utf8(payload)
        .map_err(|e| {
            AnalysisError::UnsupportedInput(format!("invalid utf-8 at byte {}", e.valid_up_to()))
        })?
        .to_string();
    let has_crlf = text.contains("\r\n");
    let bare_lf = text
        .as_bytes()
        .iter()
        .enumerate()
        .any(|(i, &b)| b == b'\n' && (i == 0 || text.as_bytes()[i - 1] != b'\r'));
    let mixed_line_endings = has_crlf && bare_lf;

    let format = match config.input_format {
        InputFormat::Markdown => FormatData::Markdown,
        InputFormat::Text => FormatData::Text,
    };

    Ok(Prepared {
        sha256,
        original_len: input.len(),
        bom_stripped,
        mixed_line_endings,
        text,
        format,
    })
}

/// Rust-source shape test for prose input, run by `analyze` AFTER extraction
/// so the prose and code split is the real extractor's segmentation.
/// `code_blocks` is the extractor's code-BLOCK region list, covering backtick
/// fences, tilde fences, and four-space indented blocks alike, so a line
/// overlapping any code block is code and never counts. A guide with a long
/// indented listing and a post with a tilde-fenced sample both stay prose.
///
/// Scope is deliberately narrow. The test reads Rust shape, and nothing else.
/// Source in another language reaches the rules and produces findings a reader
/// discounts, which is the documented trade for a guard that does not fire on
/// prose. The guard catches a mistake, and it is not a security boundary: a
/// writer who prefixes every line with a comment marker gets past it, which is
/// deliberate and recorded.
///
/// Returns `Some((code_lines, nonblank_lines))` when at least
/// `SOURCE_GUARD_MIN_LINES` lines carry code structure and they are at least
/// `SOURCE_GUARD_MIN_PCT` percent of the non-blank outside-code lines.
const SOURCE_GUARD_MIN_LINES: usize = 8;
const SOURCE_GUARD_MIN_PCT: usize = 35;

pub fn source_shape(text: &str, code_blocks: &[Range<usize>]) -> Option<(usize, usize)> {
    let (code_lines, nonblank) = source_line_counts(text, code_blocks);
    if code_lines >= SOURCE_GUARD_MIN_LINES && code_lines * 100 >= SOURCE_GUARD_MIN_PCT * nonblank {
        Some((code_lines, nonblank))
    } else {
        None
    }
}

/// The raw counts behind `source_shape`: lines carrying code structure, and
/// non-blank lines outside code blocks. Public so a measurement can score a
/// document without asking whether it trips the thresholds.
pub fn source_line_counts(text: &str, code_blocks: &[Range<usize>]) -> (usize, usize) {
    let mut code_lines = 0usize;
    let mut nonblank = 0usize;
    for lr in line_ranges(text) {
        let t = text[lr.clone()].trim();
        if t.is_empty() {
            continue;
        }
        if code_blocks
            .iter()
            .any(|c| c.start < lr.end && lr.start < c.end)
        {
            continue;
        }
        nonblank += 1;
        if code_shaped_line(t) {
            code_lines += 1;
        }
    }
    (code_lines, nonblank)
}

/// One trimmed line's Rust-shape test, in two arms.
///
/// Arm 1 needs no terminator, because the shape is unambiguous on its own: an
/// attribute or comment opener, or a line made only of structural
/// punctuation. Plain `//` comments count alongside `///` and `//!`. In
/// markdown that shape lives inside a fence, which segmentation already
/// excludes, so counting it costs no prose and closes most of the
/// comment-prefix evasion as a side effect.
///
/// Arm 2 needs both halves. The line has to end on a code terminator AND
/// either open with an item or binding keyword, optionally behind a
/// visibility or modifier word, or carry a path, arrow, or fat-arrow token,
/// or be a field line. Requiring both is what keeps prose out: a sentence
/// that opens with `use` or `type` ends on a period, and a sentence that ends
/// on a semicolon does not open with a keyword or carry `::`.
fn code_shaped_line(t: &str) -> bool {
    // Arm 1.
    if t.starts_with("#[") || t.starts_with("#![") || t.starts_with("//") {
        return true;
    }
    if t.chars()
        .all(|c| matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ','))
    {
        return true;
    }

    // Arm 2, first half: a code terminator at the line end.
    if !t.ends_with(['{', '}', ';', '(', ')', ',', ']']) {
        return false;
    }
    // Arm 2, second half: a keyword opener, a code token, or a field line.
    const KEYWORDS: &[&str] = &[
        "fn", "struct", "enum", "impl", "trait", "mod", "use", "const", "static", "type", "let",
        "match", "extern",
    ];
    const MODIFIERS: &[&str] = &["pub", "pub(crate)", "async", "unsafe"];
    let mut words = t.split_whitespace();
    let mut head = words.next().unwrap_or("");
    if MODIFIERS.contains(&head) {
        head = words.next().unwrap_or("");
    }
    let head_word: &str = head
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("");
    KEYWORDS.contains(&head_word)
        || t.contains("::")
        || t.contains("->")
        || t.contains("=>")
        || field_line(t)
}

/// The field-line shape, kept tight on purpose: one identifier, a colon, ONE
/// type expression carrying no sentence structure, then a comma, optionally
/// behind `pub` or `pub(crate)`. A definition list writes several words after
/// its colon, so `- name: the person who signed,` never matches, and a bare
/// element line (a lone token and a comma) is not this shape and is not
/// counted at all.
fn field_line(t: &str) -> bool {
    let Some(body) = t.strip_suffix(',') else {
        return false;
    };
    let body = body
        .strip_prefix("pub(crate) ")
        .or_else(|| body.strip_prefix("pub "))
        .unwrap_or(body)
        .trim();
    let Some((name, ty)) = body.split_once(':') else {
        return false;
    };
    // A path separator means the colon was not the field colon.
    if name.ends_with(':') || ty.starts_with(':') {
        return false;
    }
    let name = name.trim();
    if name.is_empty()
        || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
        || !name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return false;
    }
    // One type expression: an identifier, a path, a reference, or a generic,
    // written as a single whitespace-free token once references and generics
    // are allowed their own spaces.
    let ty = ty.trim();
    if ty.is_empty() {
        return false;
    }
    let compact: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    let spaced_words = ty.split_whitespace().count();
    // `&'a str` and `Vec<T, A>` keep their spaces, so allow a second word
    // only when the type carries generic or reference syntax.
    let syntactic = compact.contains(['<', '&', ':']);
    if spaced_words > 1 && !syntactic {
        return false;
    }
    compact.chars().all(|c| {
        c.is_alphanumeric()
            || matches!(
                c,
                '_' | ':' | '<' | '>' | '&' | '\'' | ',' | '[' | ']' | '(' | ')'
            )
    }) && compact
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || matches!(c, '_' | '&' | '(' | '['))
}

/// Byte range of each line, excluding the line terminator.
pub fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            out.push(start..end);
            start = i + 1;
        }
    }
    if start <= text.len() {
        out.push(start..text.len());
    }
    out
}
