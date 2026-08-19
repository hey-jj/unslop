//! Input contract: decoding, limits, and BOM handling. Fail-closed boundary.

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
