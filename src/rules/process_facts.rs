//! process-facts family structural rule: SLOP-F005 rationale leak, the
//! design argument or the reception instruction left in the text a reader
//! reads.
//!
//! The scan runs over the norm view (NFC, entity decode, escape resolution,
//! invisible removal, soft-break folding, U+FFFD barriers at code spans) and
//! maps spans back through the segment table the way the shared engine does.
//! Markers are matched word by word over the token sequence rather than as
//! raw byte substrings, so a marker split across a wrapped line still
//! matches and a marker inside a longer word never does. Every window is
//! bounded by the sentence it sits in, honoring the crate-wide ban on
//! unbounded scans.

use super::contrast::{is_tool_noun, phrase_at, phrase_words, shared_tool_nouns};
use super::sentence::{bare, blocks, param_words, push_norm_hit, sentences, word_tokens};
use crate::engine::{CompiledPolicy, Hit};
use crate::input::Prepared;
use crate::views::NormView;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-F005"];

/// SLOP-F005. A marker from either family, anchored to a tool noun standing
/// anywhere in the same sentence.
pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-F005") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let mut markers = param_words(rule, "design_markers");
    markers.extend(param_words(rule, "reception_markers"));
    let markers = phrase_words(&markers);
    // The anchor set lives on SLOP-C011 and is read from there, so the two
    // rules read the same closed list and can never drift apart.
    let tool_nouns = shared_tool_nouns(cp);

    let text = norm.text.as_str();
    let src = prepared.text.as_str();
    let lower = text.to_ascii_lowercase();

    for block in blocks(norm) {
        for sentence in sentences(text, &block) {
            let toks = word_tokens(&lower[sentence.clone()]);
            // The anchor decides the whole sentence. Without a tool noun the
            // markers are ordinary English and the rule has nothing to say.
            if !toks.iter().any(|t| is_tool_noun(&bare(t.1), &tool_nouns)) {
                continue;
            }
            for i in 0..toks.len() {
                for marker in &markers {
                    if !phrase_at(&toks, i, marker) {
                        continue;
                    }
                    let last = toks[i + marker.len() - 1];
                    // The span covers the marker words alone, so a comma or
                    // a period closing the clause stays out of the finding.
                    let end = last.0
                        + last
                            .1
                            .trim_end_matches(|c: char| !c.is_alphanumeric())
                            .len();
                    let span = sentence.start + toks[i].0..sentence.start + end;
                    push_norm_hit(idx, norm, src, span, hits);
                }
            }
        }
    }
}
