//! contrast family structural rule: SLOP-C007 apophatic self-definition,
//! trigger T1 — the trailing negation tag (`, not <NP>.` / `, never <NP>.`).
//! The rule's T2-T4 trigger forms are declared as bounded `patterns` on the
//! policy block and served by the shared regex engine with span mapping and
//! trigger fidelity inherited; this module implements only the tail form,
//! which is where the imperative-opener and second-person suppression logic
//! lives. E001 precedent: bounded hand-rolled scans over policy params, no
//! regex, no new dependency.
//!
//! The scan runs over the norm view (NFC, entity decode, escape resolution,
//! invisible removal, soft-break folding; prose-only with U+FFFD barriers at
//! code spans, so flanking text never fuses across a code region). Every
//! window is bounded by policy params, honoring the crate-wide ban on
//! unbounded scans. FP-safety is the design bias: every suppression doubt
//! resolves toward silence, and the one deliberate inversion — a clause
//! whose start lies beyond the walk-back window fires by default — is the
//! spec's fail-toward-candidate-report choice.

use crate::engine::{CompiledPolicy, Hit};
use crate::input::Prepared;
use crate::views::NormView;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-C007"];

fn param_str_list(rule: &crate::policy::Rule, key: &str) -> Vec<String> {
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

/// First word token of a clause: leading non-word characters (quotes,
/// brackets, barrier replacement chars) are skipped, then the maximal run of
/// alphanumerics plus apostrophes is collected, ASCII-lowercased, with the
/// typographic apostrophe folded so a norm-view `don\u{2019}t` still matches
/// the base-form deny-list entry `don't`.
fn first_token(clause: &str) -> String {
    let mut out = String::new();
    for c in clause.chars() {
        let c = if c == '\u{2019}' { '\'' } else { c };
        if c.is_alphanumeric() || c == '\'' {
            out.push(c.to_ascii_lowercase());
        } else if out.is_empty() {
            continue;
        } else {
            break;
        }
    }
    out
}

/// Word token beginning exactly at `at` (used for the interior-directive
/// check, where the position after `, ` or `then ` is already known).
fn token_at(clause_lower: &str, at: usize) -> String {
    first_token(&clause_lower[at..])
}

/// Word-bounded, case-insensitive containment of `needle` (already
/// lowercase) in `hay_lower` (already lowercase).
fn contains_word(hay_lower: &str, needle: &str) -> bool {
    let mut at = 0usize;
    while let Some(pos) = hay_lower[at..].find(needle) {
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
            return true;
        }
        at = s + 1;
    }
    false
}

/// Bounded terminal test for a `.` met during the NP scan or the clause
/// walk-back. `dot_end` is the offset just past the `.` in `text`. A period
/// followed directly by an alphanumeric character is abbreviation- or
/// number-internal (`U.S`, `3.5`): not a terminal. A period followed by a
/// bounded ASCII space/tab run and then a lowercase continuation is
/// mid-sentence punctuation (`U.S. but`, `e.g. the`): not a terminal.
/// Everything else — end of text, a line break, an uppercase/digit/quote/
/// bracket/barrier follower, a whitespace run past the parser's 8-unit
/// bound — is a terminal, exactly as before this test existed. The peek is
/// O(1) and bounded, honoring the crate-wide ban on unbounded scans.
/// Accepted false negatives (KNOWN-EDGES): chat-style prose that starts
/// sentences lowercase reads a real terminal as a continuation and stays
/// silent, and an abbreviation followed by a capitalized word (`Mr. Smith`)
/// still reads as a terminal — both resolve toward silence or the
/// pre-existing behavior, never toward a new firing surface.
pub(crate) fn period_is_terminal(text: &str, dot_end: usize) -> bool {
    let mut chars = text[dot_end..].chars();
    let Some(first) = chars.next() else {
        return true; // end of text
    };
    if first.is_alphanumeric() {
        return false; // abbreviation- or number-internal
    }
    if first != ' ' && first != '\t' {
        // Line breaks end the block; quotes, brackets, punctuation, and the
        // U+FFFD barrier all sit on the terminal side.
        return true;
    }
    // Walk at most 8 ASCII space/tab units, mirroring the tail parser's own
    // whitespace bound.
    let mut seen = 1usize;
    loop {
        match chars.next() {
            Some(' ') | Some('\t') => {
                seen += 1;
                if seen > 8 {
                    return true;
                }
            }
            Some('\n') | Some('\r') => return true, // block end
            Some(c) => return !c.is_lowercase(),
            None => return true,
        }
    }
}

/// Parse the T1 tail shape starting at the comma at `comma`: up to 8
/// whitespace characters, `not` or `never` (case-insensitive, followed by
/// 1..=8 whitespace), then an NP of 1..=`np_max` bytes containing none of
/// `!?;:,\n` (nor a U+FFFD barrier) and at least one non-whitespace
/// character (a whitespace-only "NP" is not a noun phrase), closed by a
/// terminal `.`, `!`, or `?`. A non-terminal `.` (abbreviation-internal or
/// mid-sentence per `period_is_terminal`) is legal NP content. Returns the
/// exclusive end offset of the terminal punctuation. The
/// no-interior-comma constraint is what keeps the parenthetical
/// `X, not Y, verb ...` interpolation out of scope, and a word-bounded
/// `but` anywhere in the NP rejects the tail outright: a contrastive
/// continuation (`, not in the U.S. but in Asia.`) is the not-X-but-Y pair
/// form — SLOP-C008's territory and a legitimate contrast — never a bare
/// apophatic caveat.
/// Both whitespace loops match ASCII whitespace only (space/tab/LF/CR),
/// by design: a non-ASCII space inside a contrastive tail is an
/// attacker-unrealistic vector (see KNOWN-EDGES).
fn parse_tail(text: &str, comma: usize, np_max: usize) -> Option<usize> {
    let rest = text.get(comma + 1..)?;
    let mut i = 0usize;
    for c in rest.chars().take(8) {
        if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    let after_ws = &rest[i..];
    // `get` rather than direct slicing: the byte at the cut can sit inside a
    // multi-byte character, and a directly sliced prefix would panic there.
    let kw_len = if after_ws
        .get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("never"))
    {
        5
    } else if after_ws
        .get(..3)
        .is_some_and(|s| s.eq_ignore_ascii_case("not"))
    {
        3
    } else {
        return None;
    };
    // The keyword must be followed by 1..=8 ASCII whitespace characters
    // (its right word boundary). ASCII-only by design, deliberately
    // narrower than a Unicode `\s{1,8}`: a non-ASCII space here is an
    // accepted false negative (see KNOWN-EDGES).
    let mut j = i + kw_len;
    let mut ws = 0usize;
    for c in rest[j..].chars().take(8) {
        if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
            ws += 1;
            j += c.len_utf8();
        } else {
            break;
        }
    }
    if ws == 0 {
        return None;
    }
    // NP scan: bounded, no clause punctuation, must close with a terminal,
    // and must carry at least one non-whitespace character — an empty or
    // whitespace-only span between the keyword and the terminal is not a
    // noun phrase.
    let np_start = j;
    let mut k = j;
    let mut np_has_content = false;
    for c in rest[np_start..].chars() {
        match c {
            '.' if !period_is_terminal(text, comma + 1 + k + 1) => {
                // Abbreviation-internal or mid-sentence period (`U.S.`,
                // `e.g.`): NP content, not a terminal.
                np_has_content = true;
                k += 1;
                if k - np_start > np_max {
                    return None;
                }
            }
            '.' | '!' | '?' => {
                if !np_has_content {
                    return None; // empty or whitespace-only NP
                }
                // A word-bounded `but` inside the tail means the negation
                // carries its own contrastive continuation ("not in the
                // U.S. but in Asia"): a not-X-but-Y pair, which is a
                // legitimate contrast shape and SLOP-C008's territory, not
                // a bare apophatic caveat. The comma-tail rule stays
                // silent. Bounded: the NP is at most `np_max` bytes.
                let np_lower = rest[np_start..k].to_ascii_lowercase();
                if contains_word(&np_lower, "but") {
                    return None;
                }
                return Some(comma + 1 + k + c.len_utf8());
            }
            ';' | ':' | ',' | '\n' | '\u{FFFD}' => return None,
            _ => {
                if !c.is_whitespace() {
                    np_has_content = true;
                }
                k += c.len_utf8();
                if k - np_start > np_max {
                    return None;
                }
            }
        }
    }
    None
}

/// Recover the clause start: walk back from the comma at most `window`
/// bytes to the nearest clause boundary — a line break, or terminal
/// punctuation (`.`, `!`, `?`, plus `:` per the design) followed by
/// whitespace — mirroring the engine's own block-start notion
/// (`NormView::is_block_start`) as a single bounded backward pass. A `.`
/// additionally goes through `period_is_terminal`, so an abbreviation
/// (`the U.S. market`) no longer truncates the recovered clause — the
/// suppression classifier sees the whole sentence, an FP-reducing change.
/// The `:` `!` `?` arms are untouched: a colon followed by lowercase is a
/// legitimate clause boundary and must stay one. Offset 0
/// counts as a boundary when it lies inside the window. `None` means the
/// window was exhausted without a boundary; the caller fires by default.
fn clause_start(text: &str, comma: usize, window: usize) -> Option<usize> {
    let lo = crate::widen_to_char_boundaries(text, comma.saturating_sub(window)..comma).start;
    let region = &text[lo..comma];
    for (off, c) in region.char_indices().rev() {
        let abs = lo + off;
        let boundary_end = match c {
            '\n' => Some(abs + 1),
            '.' | '!' | '?' | ':' => {
                let next = text[abs + c.len_utf8()..].chars().next();
                if matches!(next, Some(w) if w.is_whitespace())
                    && (c != '.' || period_is_terminal(text, abs + 1))
                {
                    Some(abs + c.len_utf8())
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(mut p) = boundary_end {
            // The clause proper starts after the whitespace run.
            for w in text[p..comma].chars() {
                if w.is_whitespace() {
                    p += w.len_utf8();
                } else {
                    break;
                }
            }
            return Some(p);
        }
    }
    if lo == 0 {
        return Some(0);
    }
    None
}

/// The section-3 suppression classifier over a recovered clause. True means
/// the site reads as a directive and stays silent.
fn suppressed(clause: &str, openers: &[String], second_person: &[String]) -> bool {
    let lower = clause.to_lowercase();
    // 1. Imperative opener: the clause's first token is on the base-form
    //    deny-list.
    let head = first_token(&lower);
    if !head.is_empty() && openers.contains(&head) {
        return true;
    }
    // 2. Second-person cue anywhere before the comma, word-bounded.
    if second_person.iter().any(|t| contains_word(&lower, t)) {
        return true;
    }
    // 3. A deny-list verb immediately after an interior `, ` or after
    //    `then ` — the leading-adverbial directive
    //    ("When in doubt, use the builder, not the raw constructor.").
    let mut at = 0usize;
    while let Some(pos) = lower[at..].find(", ") {
        let s = at + pos + 2;
        let tok = token_at(&lower, s);
        if !tok.is_empty() && openers.contains(&tok) {
            return true;
        }
        at = s;
    }
    let mut at = 0usize;
    while let Some(pos) = lower[at..].find("then ") {
        let s = at + pos;
        let before_ok = lower[..s]
            .chars()
            .next_back()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        if before_ok {
            let tok = token_at(&lower, s + 5);
            if !tok.is_empty() && openers.contains(&tok) {
                return true;
            }
        }
        at = s + 5;
    }
    false
}

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-C007") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let np_max = super::param_i64(rule, "tail_np_max_bytes").unwrap_or(60) as usize;
    let window = super::param_i64(rule, "clause_window_bytes").unwrap_or(240) as usize;
    let openers = param_str_list(rule, "imperative_openers");
    let second_person = param_str_list(rule, "second_person");

    let text = norm.text.as_str();
    let src = prepared.text.as_str();
    for (comma, _) in text.char_indices().filter(|(_, c)| *c == ',') {
        let Some(tail_end) = parse_tail(text, comma, np_max) else {
            continue;
        };
        // Clause recovery within the bounded window. A recovered clause goes
        // through the suppression classifier; an exhausted window fires by
        // default (spec section 3: fail toward the candidate report).
        if let Some(cs) = clause_start(text, comma, window) {
            if suppressed(&text[cs..comma], &openers, &second_person) {
                continue;
            }
        }
        let span = comma..tail_end;
        // Map exactly as accept_word_hit does: through the segment table,
        // widened against the source. Trigger fidelity re-verifies the
        // reported slice at emit, so a mapping bug fails closed as exit 30
        // instead of surfacing a finding at the wrong bytes.
        let Some(source_span) = norm.to_source(span.clone()) else {
            continue;
        };
        let source_span = crate::widen_to_char_boundaries(src, source_span);
        if source_span.start >= source_span.end {
            continue;
        }
        let mut hit = Hit::new(idx, source_span);
        hit.quoted = norm.all_quoted(&span);
        hit.trigger = Some(text[span].to_string());
        hits.push(hit);
    }
}
