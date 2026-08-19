//! Sentence-shape rules over the norm view: SLOP-M007 colon as connector,
//! SLOP-C010 false range, SLOP-O005 participial tail, SLOP-L003 dense
//! sentence, SLOP-L001 agentive passive, and SLOP-A008 metaphor nouns in the
//! of-frame.
//!
//! All four scan the norm view (NFC, entity decode, escape resolution,
//! invisible removal, soft-break folding, U+FFFD barriers at code spans) and
//! map spans back through the segment table exactly as the shared engine
//! does. Every window is bounded by a policy param or by the block it sits
//! in, honoring the crate-wide ban on unbounded scans.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::Prepared;
use crate::views::NormView;
use crate::Config;
use std::ops::Range;

pub const HANDLED: &[&str] = &[
    "SLOP-M007",
    "SLOP-C010",
    "SLOP-O005",
    "SLOP-L003",
    "SLOP-L001",
    "SLOP-A008",
];

/// Norm ranges of every block the view recorded, trailing whitespace
/// trimmed. Empty blocks are dropped. Block granularity, not line: a
/// paragraph wrapped across source lines is one block, so a sentence that
/// spans a soft break is one sentence.
pub(crate) fn blocks(norm: &NormView) -> Vec<Range<usize>> {
    let text = norm.text.as_str();
    let mut bounds: Vec<usize> = norm.block_starts.clone();
    bounds.push(text.len());
    bounds.dedup();
    let mut out = Vec::new();
    for w in bounds.windows(2) {
        let (start, end) = (w[0], w[1]);
        if start >= end || end > text.len() {
            continue;
        }
        let trimmed = text[start..end].trim_end();
        if trimmed.is_empty() {
            continue;
        }
        out.push(start..start + trimmed.len());
    }
    out
}

/// True when the `.` ending at `dot_end` closes a sentence rather than an
/// abbreviation or a number. Mirrors the contrast module's bounded test: a
/// following alphanumeric is internal, a lowercase continuation after a
/// bounded space run is mid-sentence, everything else is terminal.
fn period_is_terminal(text: &str, dot_end: usize) -> bool {
    let mut chars = text[dot_end..].chars();
    let Some(first) = chars.next() else {
        return true;
    };
    if first.is_alphanumeric() {
        return false;
    }
    if first != ' ' && first != '\t' {
        return true;
    }
    let mut seen = 1usize;
    loop {
        match chars.next() {
            Some(' ') | Some('\t') => {
                seen += 1;
                if seen > 8 {
                    return true;
                }
            }
            Some('\n') | Some('\r') => return true,
            Some(c) => return !c.is_lowercase(),
            None => return true,
        }
    }
}

/// Sentence ranges inside one block. A sentence never crosses a block
/// boundary, so a heading or list item is one unit no matter how it is
/// punctuated.
pub(crate) fn sentences(text: &str, block: &Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = block.start;
    let body = &text[block.clone()];
    for (off, c) in body.char_indices() {
        let abs = block.start + off;
        let terminal = match c {
            '.' => period_is_terminal(text, abs + 1),
            '!' | '?' => true,
            _ => false,
        };
        if !terminal {
            continue;
        }
        let end = abs + c.len_utf8();
        if text[start..end].trim().is_empty() {
            start = end;
            continue;
        }
        out.push(start..end);
        start = end;
    }
    if start < block.end && !text[start..block.end].trim().is_empty() {
        out.push(start..block.end);
    }
    out
}

/// Word-bounded containment of an already-lowercase ASCII `needle` in an
/// ASCII-lowercased haystack. ASCII-lowercasing is what makes the returned
/// offset usable against the original text: it never changes a byte length,
/// while full Unicode lowercasing can, and an offset computed against a
/// shifted string would slice mid-character.
pub(crate) fn find_word(hay_lower: &str, needle: &str, from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(pos) = hay_lower.get(at..)?.find(needle) {
        let s = at + pos;
        let e = s + needle.len();
        let before_ok = hay_lower[..s]
            .chars()
            .next_back()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        let after_ok = hay_lower[e..]
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        if before_ok && after_ok {
            return Some(s);
        }
        at = s + 1;
    }
    None
}

/// Emit one hit, mapping the norm span onto the source the way the shared
/// engine does. A span that does not map is dropped rather than reported at
/// the wrong bytes.
pub(crate) fn push_norm_hit(
    idx: usize,
    norm: &NormView,
    src: &str,
    span: Range<usize>,
    hits: &mut Vec<Hit>,
) {
    let Some(source_span) = norm.to_source(span.clone()) else {
        return;
    };
    let source_span = crate::widen_to_char_boundaries(src, source_span);
    if source_span.start >= source_span.end {
        return;
    }
    let mut hit = Hit::new(idx, source_span);
    hit.quoted = norm.all_quoted(&span);
    hit.trigger = Some(norm.text[span].to_string());
    hits.push(hit);
}

/// A policy param holding a list of lowercase strings.
pub(crate) fn param_words(rule: &crate::policy::Rule, key: &str) -> Vec<String> {
    rule.params
        .as_table()
        .and_then(|t| t.get(key))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                .collect()
        })
        .unwrap_or_default()
}

/// Whitespace-separated tokens as (offset, raw token) pairs. Offsets are
/// relative to `text`.
pub(crate) fn word_tokens(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                out.push((s, &text[s..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

/// A token with its surrounding punctuation removed, ASCII-lowercased.
pub(crate) fn bare(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_ascii_lowercase()
}

/// A quantity endpoint: a number, a date, a unit, or a quantity word. A
/// range with one of these on either side is a real range.
fn is_quantity(endpoint: &str) -> bool {
    const MONTHS: &[&str] = &[
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
    const QUANTITY_WORDS: &[&str] = &[
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "none",
        "few",
        "several",
        "many",
        "most",
        "all",
        "half",
        "dozens",
        "hundreds",
        "thousands",
        "millions",
        "billions",
        "birth",
        "dawn",
        "start",
        "beginning",
        "end",
        "today",
        "yesterday",
        "tomorrow",
        "now",
        "then",
    ];
    let lower = endpoint.to_ascii_lowercase();
    if lower.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .any(|w| MONTHS.contains(&w) || QUANTITY_WORDS.contains(&w))
}

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let text = norm.text.as_str();
    let src = prepared.text.as_str();
    let lower = text.to_ascii_lowercase();
    let blocks = blocks(norm);

    colon_as_connector(cp, doc, norm, src, &blocks, config, hits);
    false_range(cp, norm, src, &lower, &blocks, config, hits);
    participial_tail(cp, norm, src, &lower, &blocks, config, hits);
    dense_sentence(cp, norm, src, &blocks, config, hits);
    agentive_passive(cp, norm, src, &blocks, config, hits);
    metaphor_of_frame(cp, norm, src, &blocks, config, hits);
}

/// SLOP-M007. A colon joining two clauses mid-sentence.
fn colon_as_connector(
    cp: &CompiledPolicy,
    doc: &Doc,
    norm: &NormView,
    src: &str,
    blocks: &[Range<usize>],
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-M007") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let min_words = super::param_i64(rule, "min_words_before_colon").unwrap_or(3);
    let max_tail_commas = super::param_i64(rule, "max_tail_commas").unwrap_or(1) as usize;
    let text = norm.text.as_str();
    // Source offsets where a bold or italic run ends. A colon sitting there
    // is the inline-header shape SLOP-E003 owns.
    let emphasis_ends: Vec<usize> = doc.emphasis.iter().map(|(r, _)| r.end).collect();
    for block in blocks {
        for sentence in sentences(text, block) {
            let body = &text[sentence.clone()];
            for (off, c) in body.char_indices() {
                if c != ':' {
                    continue;
                }
                let abs = sentence.start + off;
                let before = &text[sentence.start..abs];
                // A letter immediately before, so a clock time, a ratio, and
                // a chapter reference never fire.
                if !before
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphabetic())
                {
                    continue;
                }
                if (before.split_whitespace().count() as i64) < min_words {
                    continue;
                }
                // A space and then a lowercase letter after: a line-final
                // colon introducing a list has no following word, and a
                // capitalized follower reads as a label.
                let after = &text[abs + 1..sentence.end];
                let mut it = after.chars();
                if it.next() != Some(' ') {
                    continue;
                }
                if !it.next().is_some_and(|c| c.is_lowercase()) {
                    continue;
                }
                // An enumeration after the colon is the list case the rule
                // exempts by design. More than one comma is a list, and so is
                // a two-item join, which needs no comma at all.
                let tail = &text[abs + 1..sentence.end];
                let tail_lower = tail.to_ascii_lowercase();
                if tail.matches(',').count() > max_tail_commas
                    || find_word(&tail_lower, "and", 0).is_some()
                    || find_word(&tail_lower, "or", 0).is_some()
                {
                    continue;
                }
                // The cataphoric form points forward at an example, which is
                // the use the rule allows.
                let last_word = bare(before.split_whitespace().next_back().unwrap_or(""));
                if matches!(last_word.as_str(), "this" | "these" | "following") {
                    continue;
                }
                if let Some(source_colon) = norm.to_source(abs..abs + 1) {
                    if emphasis_ends.contains(&source_colon.start) {
                        continue;
                    }
                }
                push_norm_hit(idx, norm, src, abs..abs + 1, hits);
            }
        }
    }
}

/// SLOP-C010. A from-X-to-Y frame whose endpoints share no scale.
fn false_range(
    cp: &CompiledPolicy,
    norm: &NormView,
    src: &str,
    lower: &str,
    blocks: &[Range<usize>],
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-C010") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let max_endpoint = super::param_i64(rule, "max_endpoint_bytes").unwrap_or(40) as usize;
    let verb_window = super::param_i64(rule, "verb_window_words").unwrap_or(4) as usize;
    let breadth = param_words(rule, "breadth_signals");
    let heads = param_words(rule, "category_heads");
    let motion = param_words(rule, "motion_verbs");
    let text = norm.text.as_str();
    for block in blocks {
        for sentence in sentences(text, block) {
            let sent_lower = &lower[sentence.clone()];
            // Arm A is a property of the whole sentence, so it is computed
            // once. Arm B is per occurrence and is checked below.
            let arm_a = breadth
                .iter()
                .any(|w| find_word(sent_lower, w, 0).is_some());
            let tokens = word_tokens(sent_lower);
            let mut at = 0usize;
            while let Some(from_rel) = find_word(sent_lower, "from", at) {
                at = from_rel + 4;
                // Suppression 1: a motion, transfer, or conversion verb in
                // the window before `from` makes this a journey or a change.
                let before: Vec<&str> = tokens
                    .iter()
                    .filter(|(o, _)| *o < from_rel)
                    .rev()
                    .take(verb_window)
                    .map(|(_, t)| *t)
                    .collect();
                if before.iter().any(|t| motion.contains(&bare(t))) {
                    continue;
                }
                let first_start = from_rel + 4;
                let window_end = crate::widen_to_char_boundaries(
                    sent_lower,
                    first_start..(first_start + max_endpoint).min(sent_lower.len()),
                )
                .end;
                let Some(to_rel) = find_word(&sent_lower[..window_end], "to", first_start) else {
                    continue;
                };
                let first = sent_lower[first_start..to_rel].trim();
                let second_start = to_rel + 2;
                let second_end = crate::widen_to_char_boundaries(
                    sent_lower,
                    second_start..(second_start + max_endpoint).min(sent_lower.len()),
                )
                .end;
                let second_raw = &sent_lower[second_start..second_end];
                let clause = second_raw
                    .split([',', ';', '.'])
                    .next()
                    .unwrap_or(second_raw);
                // The endpoint is the noun phrase after `to`, not the rest of
                // the sentence: a window that swallowed the tail let any
                // quantity word later in the sentence suppress the finding.
                let toks = word_tokens(clause);
                let second_len = toks.get(3).map(|(o, _)| *o).unwrap_or(clause.len());
                let second = clause[..second_len].trim();
                if first.is_empty() || second.is_empty() {
                    continue;
                }
                // Suppression 2: a quantity on either endpoint means the
                // scale is real.
                if is_quantity(first) || is_quantity(second) {
                    continue;
                }
                // Arm B: a category head immediately before `from`.
                let arm_b = before
                    .first()
                    .map(|t| heads.contains(&bare(t)))
                    .unwrap_or(false);
                if !arm_a && !arm_b {
                    continue;
                }
                let span_start = sentence.start + from_rel;
                let span_end = sentence.start + second_start + second.len() + 1;
                let span = span_start..span_end.min(sentence.end);
                push_norm_hit(idx, norm, src, span, hits);
                at = second_start;
            }
        }
    }
}

/// SLOP-L001. Passive voice with the actor still behind a by-phrase.
fn agentive_passive(
    cp: &CompiledPolicy,
    norm: &NormView,
    src: &str,
    blocks: &[Range<usize>],
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-L001") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let irregular = param_words(rule, "irregular_participles");
    let temporal = param_words(rule, "temporal_nouns");
    const AUX: &[&str] = &["is", "are", "was", "were", "being"];
    let text = norm.text.as_str();
    for block in blocks {
        for sentence in sentences(text, block) {
            let body = &text[sentence.clone()];
            let tokens = word_tokens(body);
            for i in 0..tokens.len() {
                let w = bare(tokens[i].1);
                // `is/are/was/were/being X by` or `has/have/had been X by`.
                let participle_at = if AUX.contains(&w.as_str()) {
                    i + 1
                } else if matches!(w.as_str(), "has" | "have" | "had")
                    && tokens.get(i + 1).map(|t| bare(t.1)) == Some("been".to_string())
                {
                    i + 2
                } else {
                    continue;
                };
                let Some((_, praw)) = tokens.get(participle_at) else {
                    continue;
                };
                let participle = bare(praw);
                let regular = participle.len() >= 4
                    && (participle.ends_with("ed") || participle.ends_with("en"));
                if !regular && !irregular.contains(&participle) {
                    continue;
                }
                if tokens.get(participle_at + 1).map(|t| bare(t.1)) != Some("by".to_string()) {
                    continue;
                }
                let Some((actor_off, actor_raw)) = tokens.get(participle_at + 2) else {
                    continue;
                };
                let actor = bare(actor_raw);
                // A by-phrase naming a time is a deadline, not an actor.
                if actor.is_empty()
                    || temporal.contains(&actor)
                    || actor.chars().next().is_some_and(|c| c.is_ascii_digit())
                {
                    continue;
                }
                let span = sentence.start + tokens[i].0
                    ..sentence.start + actor_off + actor_raw.trim_end_matches(['.', ',']).len();
                push_norm_hit(idx, norm, src, span, hits);
            }
        }
    }
}

/// SLOP-A008. A metaphor noun inside the of-frame.
fn metaphor_of_frame(
    cp: &CompiledPolicy,
    norm: &NormView,
    src: &str,
    blocks: &[Range<usize>],
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-A008") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let within = super::param_i64(rule, "of_within_tokens").unwrap_or(2) as usize;
    let terms: Vec<String> = rule.terms.iter().map(|t| t.to_ascii_lowercase()).collect();
    const DETERMINERS: &[&str] = &["the", "a", "an", "its", "our", "their", "his", "her"];
    let text = norm.text.as_str();
    for block in blocks {
        for sentence in sentences(text, block) {
            let body = &text[sentence.clone()];
            let tokens = word_tokens(body);
            for i in 1..tokens.len() {
                let word = bare(tokens[i].1);
                let singular = word.strip_suffix('s').unwrap_or(&word);
                if !terms.iter().any(|t| t == &word || t == singular) {
                    continue;
                }
                if !DETERMINERS.contains(&bare(tokens[i - 1].1).as_str()) {
                    continue;
                }
                let of_at = (i + 1..=(i + within).min(tokens.len().saturating_sub(1)))
                    .find(|&j| bare(tokens[j].1) == "of");
                let Some(j) = of_at else {
                    continue;
                };
                let span = sentence.start + tokens[i - 1].0
                    ..sentence.start + tokens[j].0 + tokens[j].1.len();
                push_norm_hit(idx, norm, src, span, hits);
            }
        }
    }
}

/// SLOP-O005. A comma-opened participial clause that closes its block.
fn participial_tail(
    cp: &CompiledPolicy,
    norm: &NormView,
    src: &str,
    lower: &str,
    blocks: &[Range<usize>],
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-O005") else {
        return;
    };
    let terms: Vec<String> = cp.pkg.rules[idx]
        .terms
        .iter()
        .map(|t| t.to_ascii_lowercase())
        .collect();
    for block in blocks {
        let body = &lower[block.clone()];
        for (off, c) in body.char_indices() {
            if c != ',' {
                continue;
            }
            let abs = block.start + off;
            let rest = &lower[abs + 1..block.end];
            let ws = rest.len() - rest.trim_start().len();
            let head = rest.trim_start();
            let Some(term) = terms.iter().find(|t| {
                head.starts_with(t.as_str()) && !head[t.len()..].starts_with(char::is_alphanumeric)
            }) else {
                continue;
            };
            // Block-final: nothing after the tail but its own terminal mark.
            let tail_start = abs + 1 + ws + term.len();
            let tail = &lower[tail_start..block.end];
            let trimmed = tail.trim_end();
            let inner = trimmed.strip_suffix(['.', '!', '?']).unwrap_or(trimmed);
            if inner.contains(['.', '!', '?']) {
                continue;
            }
            push_norm_hit(idx, norm, src, abs..block.end, hits);
            break;
        }
    }
}

/// SLOP-L003. A sentence the reader has to hold open.
fn dense_sentence(
    cp: &CompiledPolicy,
    norm: &NormView,
    src: &str,
    blocks: &[Range<usize>],
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-L003") else {
        return;
    };
    let text = norm.text.as_str();
    let rule = &cp.pkg.rules[idx];
    let max_words = super::param_i64(rule, "max_words").unwrap_or(45) as usize;
    let max_commas = super::param_i64(rule, "max_clause_commas").unwrap_or(4) as usize;
    for block in blocks {
        for sentence in sentences(text, block) {
            let body = &text[sentence.clone()];
            let words = body.split_whitespace().count();
            let mut commas = 0usize;
            let mut depth = 0i32;
            let mut prev = ' ';
            let mut chars = body.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth = (depth - 1).max(0),
                    // A comma inside a number (1,024) joins digits and is
                    // not a clause boundary.
                    ',' if depth == 0
                        && !(prev.is_ascii_digit()
                            && chars.peek().is_some_and(|n| n.is_ascii_digit())) =>
                    {
                        commas += 1
                    }
                    _ => {}
                }
                prev = c;
            }
            // An enumeration is a list, not a stack of clauses. Two shapes
            // say so: a run that closes with "and X" or "or X", and a comma
            // rate no clause structure could produce, since a clause needs
            // at least three words. Either one leaves only length.
            let enumeration = body.contains(", and ")
                || body.contains(", or ")
                || (commas > 0 && words / commas < 3);
            if words > max_words || (commas >= max_commas && !enumeration) {
                let mut hit = Hit::new(idx, 0..0);
                let Some(source_span) = norm.to_source(sentence.clone()) else {
                    continue;
                };
                let source_span = crate::widen_to_char_boundaries(src, source_span);
                if source_span.start >= source_span.end {
                    continue;
                }
                hit.span = source_span;
                hit.detail = Some(format!("{words} words, {commas} clause commas"));
                hits.push(hit);
            }
        }
    }
}
