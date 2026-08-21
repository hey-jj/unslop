//! coverage family: SLOP-H001 section map (carried in the coverage block of
//! every report), SLOP-H002 segmentation residue, SLOP-H003 encoding
//! oddities. Hints never gate.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::{Doc, RegionKind};
use crate::input::Prepared;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-H001", "SLOP-H002", "SLOP-H003"];

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    // SLOP-H001 is instrumentation output: the section map is always present
    // in the coverage block, so it emits no finding of its own.

    if let Some(idx) = super::active(cp, config, "SLOP-H002") {
        let rule = &cp.pkg.rules[idx];
        let flag_pct = super::param_i64(rule, "flag_excluded_pct").unwrap_or(40) as usize;
        let total = prepared.text.len();
        let prose: usize = doc
            .regions
            .iter()
            .filter(|r| r.kind == RegionKind::Prose)
            .map(|r| r.range.len())
            .sum();
        if total > 0 && (total - prose) * 100 >= flag_pct * total {
            hits.push(Hit::new(idx, crate::first_char_span(&prepared.text)));
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-H003") {
        if prepared.mixed_line_endings || prepared.bom_stripped {
            hits.push(Hit::new(idx, crate::first_char_span(&prepared.text)));
        }
        // unusual_scripts_in_identifierlike_prose: a mixed-script token
        // — Latin letters sharing a word with a cross-script homoglyph — is
        // worth a human glance whether or not the folded form matches a
        // lexicon word. The norm-view fold (views.rs) is what closes the
        // evasion via the lexicon rules; this hint surfaces the oddity
        // itself. First occurrence only.
        'scan: for (range, _) in &doc.prose_regions {
            let slice = &prepared.text[range.clone()];
            let mut i = 0usize;
            while i < slice.len() {
                let ch = slice[i..].chars().next().unwrap_or('\u{FFFD}');
                if !ch.is_alphanumeric() {
                    i += ch.len_utf8();
                    continue;
                }
                // The maximal alphanumeric token starting here. The
                // mixed-script verdict is computed ONCE per token; deciding
                // it per confusable char re-walked the token each time and
                // made one giant single-script confusable run O(token^2).
                let tlen: usize = slice[i..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric())
                    .map(|c| c.len_utf8())
                    .sum();
                let token = &slice[i..i + tlen];
                if token.chars().any(|c| c.is_ascii_alphabetic()) {
                    // Mixed-script token: hit its first confusable, exactly
                    // the char the per-char scan reported.
                    if let Some((off, c)) = token
                        .char_indices()
                        .find(|&(_, c)| crate::views::confusable_latin(c).is_some())
                    {
                        let at = range.start + i + off;
                        hits.push(Hit::new(idx, at..at + c.len_utf8()));
                        break 'scan;
                    }
                }
                i += tlen;
            }
        }
    }
}
