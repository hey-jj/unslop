//! Position- and shape-anchored framing rules: SLOP-O007 name-dropping and
//! SLOP-O008 generic conclusion. Both scan the norm view and both are
//! anchored, because the words alone are ordinary: three capitalized names
//! are a fact until an attribution trigger turns them into a credential
//! list, and a stock closing line is a passing remark until it becomes the
//! last thing the reader sees.

use crate::engine::{CompiledPolicy, Hit};
use crate::input::Prepared;
use crate::views::NormView;
use crate::Config;

use super::sentence::{blocks, find_word, param_words, push_norm_hit, sentences};

pub const HANDLED: &[&str] = &["SLOP-O007", "SLOP-O008"];

/// A capitalized token that names a date rather than an outlet. A weekday or
/// a month is capitalized in every sentence that has one and never belongs to
/// the list this rule counts.
const CALENDAR: &[&str] = &[
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// Count of capitalized items in a slice of a sentence. Consecutive
/// capitalized words are one item, so "The New York Times" counts once, and
/// a separator closes the item, so "Wired, The Atlantic" counts twice.
/// Calendar words never count.
fn capitalized_items(slice: &str) -> usize {
    let mut items = 0usize;
    let mut in_item = false;
    for token in slice.split_whitespace() {
        let word = token.trim_matches(|c: char| !c.is_alphanumeric());
        let capitalized = word.chars().next().is_some_and(char::is_uppercase)
            && word.len() > 1
            && !CALENDAR.contains(&word.to_ascii_lowercase().as_str());
        if capitalized {
            if !in_item {
                items += 1;
                in_item = true;
            }
        } else {
            in_item = false;
        }
        if token.ends_with([',', ';', '.', ':']) {
            in_item = false;
        }
    }
    items
}

/// True when the block reads as a sign-off rather than as writing: it opens
/// with a valediction or byline term, or it is a short line with no sentence
/// punctuation, which is what a name on its own line looks like.
fn is_signoff(block: &str, valedictions: &[String]) -> bool {
    let trimmed = block.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    if valedictions.iter().any(|v| lower.starts_with(v.as_str())) {
        return true;
    }
    let words = trimmed.split_whitespace().count();
    words <= 4 && !trimmed.ends_with(['.', '!', '?'])
}

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let text = norm.text.as_str();
    let src = prepared.text.as_str();
    // ASCII-lowercased so every offset stays valid against `text`.
    let lower = text.to_ascii_lowercase();
    let blocks = blocks(norm);

    if let Some(idx) = super::active(cp, config, "SLOP-O007") {
        let rule = &cp.pkg.rules[idx];
        let triggers: Vec<String> = rule
            .params
            .as_table()
            .and_then(|t| t.get("triggers"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                    .collect()
            })
            .unwrap_or_default();
        let after_triggers = param_words(rule, "triggers_after");
        let min_items = super::param_i64(rule, "min_capitalized_items").unwrap_or(3) as usize;
        for block in &blocks {
            for sentence in sentences(text, block) {
                let body = &text[sentence.clone()];
                // A sentence that quotes a source is doing the work the rule
                // asks for.
                if body.contains(['"', '\u{201C}', '\u{201D}']) {
                    continue;
                }
                let sent_lower = &lower[sentence.clone()];
                // The list can sit on either side of the attribution. A
                // trigger phrase takes the list after it, and a reporting
                // verb takes the list before it.
                let after = triggers
                    .iter()
                    .filter_map(|t| find_word(sent_lower, t, 0).map(|p| p + t.len()))
                    .min()
                    .map(|at| capitalized_items(&body[at..]))
                    .unwrap_or(0);
                let before = after_triggers
                    .iter()
                    .filter_map(|t| find_word(sent_lower, t, 0))
                    .min()
                    .map(|at| capitalized_items(&body[..at]))
                    .unwrap_or(0);
                if after.max(before) >= min_items {
                    push_norm_hit(idx, norm, src, sentence.clone(), hits);
                }
            }
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-O008") {
        let terms: Vec<String> = cp.pkg.rules[idx]
            .terms
            .iter()
            .map(|t| t.to_ascii_lowercase())
            .collect();
        // The valediction lexicon belongs to SLOP-S004, and reading it here
        // keeps one list rather than two. A sign-off after the ending does
        // not move where the ending is.
        let valedictions: Vec<String> = cp
            .pkg
            .rule_by_id("SLOP-S004")
            .map(|r| r.terms.iter().map(|t| t.to_ascii_lowercase()).collect())
            .unwrap_or_default();
        let last = blocks
            .iter()
            .rev()
            .find(|b| !is_signoff(&text[(*b).clone()], &valedictions));
        if let Some(last) = last {
            let block_lower = &lower[last.clone()];
            for term in &terms {
                if let Some(pos) = find_word(block_lower, term, 0) {
                    let start = last.start + pos;
                    push_norm_hit(idx, norm, src, start..start + term.len(), hits);
                    break;
                }
            }
        }
    }
}
