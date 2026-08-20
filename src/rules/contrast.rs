//! contrast family structural rules. SLOP-C007 apophatic self-definition,
//! trigger T1, is the trailing negation tag (`, not <NP>.` /
//! `, never <NP>.`). SLOP-C011 proleptic capability denial reads clauses and
//! their neighbouring sentences inside one block, and its section starts at
//! the marked divider below.
//! The C007 T2-T4 trigger forms are declared as bounded `patterns` on the
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

use super::sentence::{bare, blocks, param_words, push_norm_hit, sentences, word_tokens};
use crate::engine::{CompiledPolicy, Hit};
use crate::input::Prepared;
use crate::views::NormView;
use crate::Config;
use std::ops::Range;

pub const HANDLED: &[&str] = &["SLOP-C007", "SLOP-C011"];

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
    apophatic_tail(cp, prepared, norm, config, hits);
    capability_denial(cp, prepared, norm, config, hits);
}

/// SLOP-C007 trigger T1: the trailing negation tag.
fn apophatic_tail(
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

// ---------------------------------------------------------------------------
// SLOP-C011 proleptic capability denial
// ---------------------------------------------------------------------------

/// The one tool-noun set, declared on SLOP-C011 and read from there by every
/// rule that needs it. Two copies of a closed set drift; one does not.
pub(crate) fn shared_tool_nouns(cp: &CompiledPolicy) -> Vec<String> {
    match super::rule_idx(cp, "SLOP-C011") {
        Some(idx) => param_words(&cp.pkg.rules[idx], "tool_nouns"),
        None => Vec::new(),
    }
}

/// Coordinators that may stand in front of a clause subject. They are
/// skipped once per clause, before every test, so the exclusion and the
/// subject match read the same starting token.
const COORDINATORS: &[&str] = &["and", "or", "but", "yet", "so", "nor"];

/// One clause and the segments the family tests read. The clause is what
/// commas and semicolons cut, and it is the unit a finding reports, so a
/// span never lands mid-phrase. The segments are the clause cut again at
/// every interior coordinator, and they are what the family tests read: a
/// writer who drops the comma before a joined fragment leaves
/// `X does not detect authorship and never scores voice` inside one clause,
/// and one clause cannot be two denials. One pass produces both.
struct Clause {
    range: Range<usize>,
    segments: Vec<Range<usize>>,
}

fn clause_ranges(text: &str, sentence: &Range<usize>, terms: &DenialTerms) -> Vec<Clause> {
    let mut out = Vec::new();
    let mut push = |range: Range<usize>| {
        if text[range.clone()].trim().is_empty() {
            return;
        }
        let segments = segment_at_coordinators(text, &range);
        out.push(Clause { range, segments });
    };
    let mut start = sentence.start;
    for (off, c) in text[sentence.clone()].char_indices() {
        let abs = sentence.start + off;
        // A period the sentence splitter read as mid-sentence punctuation
        // still ends a clause when a product name follows it, since a name
        // written lowercase is what made the splitter keep reading.
        if c == '.' && starts_product_name(text, abs + 1, terms) {
            push(start..abs);
            start = abs + 1;
            continue;
        }
        if c != ',' && c != ';' {
            continue;
        }
        push(start..abs);
        start = abs + c.len_utf8();
    }
    if start < sentence.end {
        push(start..sentence.end);
    }
    out
}

/// True when the text at `at`, past its whitespace, opens on a product name.
fn starts_product_name(text: &str, at: usize, terms: &DenialTerms) -> bool {
    text.get(at..)
        .and_then(|rest| rest.split_whitespace().next())
        .is_some_and(|word| terms.product_names.contains(&bare(word)))
}

/// The clause cut at every interior coordinator, each coordinator opening the
/// segment after it, where the leading-coordinator skip already reads it.
fn segment_at_coordinators(text: &str, clause: &Range<usize>) -> Vec<Range<usize>> {
    let lower = text[clause.clone()].to_ascii_lowercase();
    let toks = word_tokens(&lower);
    let mut cuts: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(i, (_, tok))| *i > 0 && COORDINATORS.contains(&bare(tok).as_str()))
        .map(|(_, (off, _))| clause.start + off)
        .collect();
    if cuts.is_empty() {
        return vec![clause.clone()];
    }
    cuts.push(clause.end);
    let mut out = Vec::new();
    let mut start = clause.start;
    for cut in cuts {
        if !text[start..cut].trim().is_empty() {
            out.push(start..cut);
        }
        start = cut;
    }
    out
}

/// Policy phrases split into words. Matching runs word by word rather than
/// over raw bytes, so a phrase broken across a wrapped line still matches.
pub(crate) fn phrase_words(list: &[String]) -> Vec<Vec<String>> {
    list.iter()
        .map(|p| p.split_whitespace().map(|w| w.to_string()).collect())
        .collect()
}

/// True when `phrase` occupies the tokens beginning at `i`, with no wildcard
/// in play. Callers holding a starred phrase use `phrase_match` instead.
pub(crate) fn phrase_at(toks: &[(usize, &str)], i: usize, phrase: &[String]) -> bool {
    phrase
        .iter()
        .enumerate()
        .all(|(k, w)| toks.get(i + k).is_some_and(|t| &bare(t.1) == w))
}

/// How many tokens a `*` may stand for. Two covers the noun phrases a writer
/// puts under a quantifier, as in no single finding. A three-token phrase is
/// a recorded miss, not a widening.
const WILDCARD_MAX_TOKENS: usize = 2;

/// Match `phrase` at `i`, where a `*` stands for one or two tokens. Returns
/// the index of the last token the wildcard covered, which is the head noun
/// of the phrase it quantified, or `None` when the phrase does not match.
pub(crate) fn phrase_match(
    toks: &[(usize, &str)],
    i: usize,
    phrase: &[String],
) -> Option<Option<usize>> {
    let Some(star) = phrase.iter().position(|w| w == "*") else {
        return phrase_at(toks, i, phrase).then_some(None);
    };
    if !phrase_at(toks, i, &phrase[..star]) {
        return None;
    }
    for width in 1..=WILDCARD_MAX_TOKENS {
        let head = i + star + width - 1;
        if toks.get(head).is_none() {
            break;
        }
        if phrase_at(toks, head + 1, &phrase[star + 1..]) {
            return Some(Some(head));
        }
    }
    None
}

/// A tool noun. The policy set carries the singular and the plural of every
/// entry, so no suffix folding runs here.
pub(crate) fn is_tool_noun(word: &str, tool_nouns: &[String]) -> bool {
    tool_nouns.iter().any(|t| t == word)
}

/// The closed-set values family 1 reads, gathered once per document.
pub(crate) struct DenialTerms {
    pub pronouns: Vec<String>,
    pub product_names: Vec<String>,
    pub tool_nouns: Vec<String>,
    /// Negations that can head a command as well as a statement.
    pub imperative_negations: Vec<Vec<String>>,
    /// Negations carrying a finite verb, which need a subject and so never
    /// head a command.
    pub finite_negations: Vec<Vec<String>>,
    pub verbs_base: Vec<String>,
    pub verbs_s: Vec<String>,
    pub verbs_ing: Vec<String>,
    pub negation_window: usize,
    pub verb_window: usize,
    pub negative_subject_window: usize,
}

impl DenialTerms {
    /// A capability verb in any form, which is what spellings A and B read.
    fn any_verb(&self, word: &str) -> bool {
        self.verbs_base.iter().any(|v| v == word)
            || self.verbs_s.iter().any(|v| v == word)
            || self.verbs_ing.iter().any(|v| v == word)
    }

    /// The last token of any negation beginning at `at`.
    fn negation_end(&self, toks: &[(usize, &str)], at: usize) -> Option<usize> {
        self.imperative_negations
            .iter()
            .chain(self.finite_negations.iter())
            .find(|n| phrase_at(toks, at, n))
            .map(|n| at + n.len() - 1)
    }
}

/// The token index a clause's tests start from: past a leading coordinator,
/// which belongs to the join and not to the clause.
fn clause_head(toks: &[(usize, &str)]) -> usize {
    usize::from(
        toks.first()
            .is_some_and(|t| COORDINATORS.contains(&bare(t.1).as_str())),
    )
}

/// A base-form verb, read by shape: an -s, -ed, or -ing ending is inflected
/// and the clause carrying it is declarative. A double-s ending (discuss,
/// address) is a base form.
fn is_base_form(word: &str) -> bool {
    !(word.ends_with('s') && !word.ends_with("ss"))
        && !word.ends_with("ed")
        && !word.ends_with("ing")
}

/// The token a negation governs, one adverb allowed in between.
fn governed_verb(toks: &[(usize, &str)], neg_end: usize) -> Option<String> {
    let mut at = neg_end + 1;
    if toks.get(at).is_some_and(|t| bare(t.1).ends_with("ly")) {
        at += 1;
    }
    toks.get(at).map(|t| bare(t.1))
}

/// The imperative test, run per clause before either family. Only an
/// imperative-capable negation can head a command, and it is one when the verb
/// it governs is a base form ("Do not obey", "Never cite", "Never score
/// voice"). The same negation over a third-person verb is a statement with its
/// subject left out ("never scores voice"), which is the middle of a denial
/// stack and stays in. A finite negation carries its subject in the verb, so a
/// clause it heads is never a command and never excluded here.
fn is_imperative(toks: &[(usize, &str)], head: usize, terms: &DenialTerms) -> bool {
    let Some(neg) = terms
        .imperative_negations
        .iter()
        .find(|n| phrase_at(toks, head, n))
    else {
        return false;
    };
    governed_verb(toks, head + neg.len() - 1).is_some_and(|w| is_base_form(&w))
}

/// A positive clause subject, returning the tokens it spans: a pronoun, a
/// product name, or `the` with a tool noun.
fn positive_subject(toks: &[(usize, &str)], at: usize, terms: &DenialTerms) -> Option<usize> {
    let head = bare(toks.get(at)?.1);
    if terms.pronouns.contains(&head) || terms.product_names.contains(&head) {
        return Some(1);
    }
    if head == "the"
        && toks
            .get(at + 1)
            .is_some_and(|t| is_tool_noun(&bare(t.1), &terms.tool_nouns))
    {
        return Some(2);
    }
    None
}

/// A negative clause subject, returning the tokens it spans: `no` with a tool
/// noun, a bare `nothing`, or `none of the` with a tool noun.
fn negative_subject(toks: &[(usize, &str)], at: usize, terms: &DenialTerms) -> Option<usize> {
    let head = bare(toks.get(at)?.1);
    if head == "nothing" {
        return Some(1);
    }
    if head == "no"
        && toks
            .get(at + 1)
            .is_some_and(|t| is_tool_noun(&bare(t.1), &terms.tool_nouns))
    {
        return Some(2);
    }
    if head == "none"
        && toks.get(at + 1).is_some_and(|t| bare(t.1) == "of")
        && toks.get(at + 2).is_some_and(|t| bare(t.1) == "the")
        && toks
            .get(at + 3)
            .is_some_and(|t| is_tool_noun(&bare(t.1), &terms.tool_nouns))
    {
        return Some(4);
    }
    None
}

/// Which shape of family 1 matched. The elided spelling is tracked apart
/// because a fragment with no subject of its own borrows the subject beside
/// it, which is what settles its coreference.
#[derive(PartialEq)]
enum Spelling {
    Subject,
    Elided,
}

/// Clause family 1: a denied capability, in the three ruled spellings. The
/// capability verb is what separates a denial of a power nobody claimed from
/// an honest scope fact, so a clause without one never qualifies.
fn denied_capability(
    toks: &[(usize, &str)],
    start: usize,
    terms: &DenialTerms,
) -> Option<Spelling> {
    let verb_at = |i: usize| toks.get(i).is_some_and(|t| terms.any_verb(&bare(t.1)));

    // Spelling A: positive subject, explicit negation, capability verb.
    if let Some(len) = positive_subject(toks, start, terms) {
        let from = start + len;
        for i in from..(from + terms.negation_window).min(toks.len()) {
            let Some(neg_end) = terms.negation_end(toks, i) else {
                continue;
            };
            if (neg_end + 1..=neg_end + terms.verb_window).any(verb_at) {
                return Some(Spelling::Subject);
            }
        }
    }
    // Spelling B: negative subject and capability verb.
    if let Some(len) = negative_subject(toks, start, terms) {
        let from = start + len;
        if (from..from + terms.negative_subject_window).any(verb_at) {
            return Some(Spelling::Subject);
        }
    }
    // Spelling C: the fragment whose subject was left out. A finite negation
    // takes a capability verb in any finite form. An imperative-capable
    // negation takes the third person only, which is the form no command
    // uses, so a command never reaches this arm. An -ing form is a
    // participial adjunct under either negation and stays out.
    let third_person_only = |i: usize| {
        toks.get(i)
            .is_some_and(|t| terms.verbs_s.iter().any(|v| *v == bare(t.1)))
    };
    let finite_form = |i: usize| {
        toks.get(i).is_some_and(|t| {
            let w = bare(t.1);
            terms.verbs_base.contains(&w) || terms.verbs_s.contains(&w)
        })
    };
    if let Some(neg) = terms
        .finite_negations
        .iter()
        .find(|n| phrase_at(toks, start, n))
    {
        let from = start + neg.len();
        if (from..from + terms.verb_window).any(finite_form) {
            return Some(Spelling::Elided);
        }
    }
    if let Some(neg) = terms
        .imperative_negations
        .iter()
        .find(|n| phrase_at(toks, start, n))
    {
        let from = start + neg.len();
        if (from..from + terms.verb_window).any(third_person_only) {
            return Some(Spelling::Elided);
        }
    }
    None
}

/// Clause family 2: an evidential hedge from the closed list, plus the open
/// form where something counted is called evidence under a preceding no.
/// The hedge a clause carries, if any. A starred entry also reports the head
/// noun it quantified, which is the token the closed-set subject test reads.
fn hedged(toks: &[(usize, &str)], hedges: &[Vec<String>]) -> Option<Option<usize>> {
    (0..toks.len()).find_map(|i| hedges.iter().find_map(|h| phrase_match(toks, i, h)))
}

/// What one segment is to the rule, computed once. A denied capability
/// reports the segment it sits in, which is the edit the writer makes. An
/// evidential hedge reports the whole comma-delimited clause, because the
/// phrase it matched can run across an interior coordinator.
struct ClauseFacts {
    segment: Range<usize>,
    clause: Range<usize>,
    /// A denial, by either family. These are what report.
    qualifying: bool,
    /// True when the denial is family 1, which decides the reported span.
    denied_capability: bool,
    /// True when the denial can complete the single-clause trigger. A family
    /// 2 clause needs a closed-set subject of its own for coreference to be
    /// testable at all, so one with a foreign subject counts toward a stack
    /// and never stands alone.
    arm_b_eligible: bool,
    /// True when the denial left its subject out and borrows the one beside
    /// it, which is what settles its coreference.
    elided_subject: bool,
    /// An affirmative self-description that can complete the single-clause
    /// trigger: a closed-set subject, no negation, no denial of its own. The
    /// degenerate restatement (`It reads text.`) is its narrowest case.
    partner: bool,
    /// The clause subject is a bare pronoun, which settles coreference alone.
    pronoun_subject: bool,
    /// The things the clause names: tool-noun lemmas with singular and plural
    /// folded to one, plus any product name. Two clauses corefer on a shared
    /// entry, never on two different nouns that both happen to be tool nouns.
    referents: Vec<String>,
}

fn clause_facts(
    clause: Range<usize>,
    segment: Range<usize>,
    lower: &str,
    terms: &DenialTerms,
    hedges: &[Vec<String>],
) -> ClauseFacts {
    let toks = word_tokens(&lower[segment.clone()]);
    let head = clause_head(&toks);
    let imperative = is_imperative(&toks, head, terms);
    let spelling = if imperative {
        None
    } else {
        denied_capability(&toks, head, terms)
    };
    let capability = spelling.is_some();
    let hedge = if imperative || capability {
        None
    } else {
        hedged(&toks, hedges)
    };
    let negated = (0..toks.len()).any(|i| terms.negation_end(&toks, i).is_some());
    let subject = positive_subject(&toks, head, terms);
    // The closed-set subject of a hedged clause is its own subject, or, where
    // the hedge quantified a noun phrase, the head noun of that phrase.
    let head_noun_is_closed = hedge.flatten().is_some_and(|at| {
        toks.get(at)
            .is_some_and(|t| is_tool_noun(&bare(t.1), &terms.tool_nouns))
    });
    let closed_subject =
        subject.is_some() || negative_subject(&toks, head, terms).is_some() || head_noun_is_closed;
    ClauseFacts {
        segment,
        clause,
        qualifying: capability || hedge.is_some(),
        denied_capability: capability,
        arm_b_eligible: capability || closed_subject,
        elided_subject: spelling == Some(Spelling::Elided),
        // An affirmative partner carries a subject and no negation at all, so
        // a second denial in the same sentence can never stand in for one.
        partner: subject.is_some() && !negated && !capability && hedge.is_none(),
        pronoun_subject: toks
            .get(head)
            .is_some_and(|t| terms.pronouns.contains(&bare(t.1))),
        referents: toks
            .iter()
            .filter_map(|t| {
                let word = bare(t.1);
                if terms.product_names.contains(&word) {
                    return Some(word);
                }
                tool_lemma(&word, &terms.tool_nouns)
            })
            .collect(),
    }
}

/// The singular form of a tool noun, so that one lemma covers both numbers.
fn tool_lemma(word: &str, tool_nouns: &[String]) -> Option<String> {
    if !is_tool_noun(word, tool_nouns) {
        return None;
    }
    match word.strip_suffix('s') {
        Some(singular) if tool_nouns.iter().any(|t| t == singular) => Some(singular.to_string()),
        _ => Some(word.to_string()),
    }
}

/// Two clauses speak about the same thing when either uses a bare pronoun, or
/// when both name the same thing, which is one tool-noun lemma in either
/// number or one product name. Two different tool nouns are two different
/// things, whatever else they have in common.
fn corefer(denial: &ClauseFacts, partner: &ClauseFacts) -> bool {
    denial.elided_subject
        || partner.pronoun_subject
        || denial.pronoun_subject
        || denial
            .referents
            .iter()
            .any(|l| partner.referents.contains(l))
}

/// SLOP-C011. A denial of a capability nobody claimed, standing next to a
/// restatement of what the thing already said it does.
fn capability_denial(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-C011") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let terms = DenialTerms {
        pronouns: param_words(rule, "subject_pronouns"),
        product_names: param_words(rule, "product_names"),
        tool_nouns: shared_tool_nouns(cp),
        imperative_negations: phrase_words(&param_words(rule, "imperative_negations")),
        finite_negations: phrase_words(&param_words(rule, "finite_negations")),
        verbs_base: param_words(rule, "capability_verbs_base"),
        verbs_s: param_words(rule, "capability_verbs_s"),
        verbs_ing: param_words(rule, "capability_verbs_ing"),
        negation_window: super::param_i64(rule, "negation_window_tokens").unwrap_or(4) as usize,
        verb_window: super::param_i64(rule, "verb_window_tokens").unwrap_or(3) as usize,
        negative_subject_window: super::param_i64(rule, "negative_subject_window_tokens")
            .unwrap_or(4) as usize,
    };
    let hedges = phrase_words(&param_words(rule, "hedges"));

    let text = norm.text.as_str();
    let src = prepared.text.as_str();
    // ASCII lowercasing never changes a byte length, so offsets taken here
    // stay usable against the norm text.
    let lower = text.to_ascii_lowercase();

    for block in blocks(norm) {
        // One segmentation pass serves every test in the rule.
        let per_sentence: Vec<Vec<ClauseFacts>> = sentences(text, &block)
            .iter()
            .map(|sentence| {
                clause_ranges(text, sentence, &terms)
                    .into_iter()
                    .flat_map(|clause| {
                        clause
                            .segments
                            .into_iter()
                            .map(move |segment| (clause.range.clone(), segment))
                    })
                    .map(|(clause, segment)| clause_facts(clause, segment, &lower, &terms, &hedges))
                    .collect()
            })
            .collect();
        let qualifying: Vec<(usize, usize)> = per_sentence
            .iter()
            .enumerate()
            .flat_map(|(si, cs)| {
                cs.iter()
                    .enumerate()
                    .filter(|(_, c)| c.qualifying)
                    .map(move |(ci, _)| (si, ci))
            })
            .collect();

        // Two qualifying clauses in the block stand on their own. A single
        // clause needs an affirmative partner speaking about the same thing,
        // searched in the ruled order: the rest of its own sentence first with
        // no distance limit, then the sentence before, then the one after.
        let fires = match qualifying.len() {
            0 => false,
            1 => {
                let (si, ci) = qualifying[0];
                let denial = &per_sentence[si][ci];
                if !denial.arm_b_eligible {
                    false
                } else {
                    let same_sentence = per_sentence[si]
                        .iter()
                        .enumerate()
                        .any(|(cj, c)| cj != ci && c.partner && corefer(denial, c));
                    same_sentence
                        || [si.checked_sub(1), Some(si + 1)]
                            .into_iter()
                            .flatten()
                            .filter_map(|sj| per_sentence.get(sj))
                            .any(|cs| cs.iter().any(|c| c.partner && corefer(denial, c)))
                }
            }
            _ => true,
        };
        if !fires {
            continue;
        }
        // One finding per qualifying clause. Each is a separate edit with its
        // own judge question, so a stack of three reaches the writer as three
        // spans rather than one span covering the sentence. The message names
        // the arm that fired, and an Arm A message carries the count, since
        // what makes a stack a stack is how many denials share the block.
        let detail = match qualifying.len() {
            1 => "arm B, one denial beside an affirmative partner".to_string(),
            n => format!("arm A, {n} denials in this block"),
        };
        for (si, ci) in qualifying {
            let facts = &per_sentence[si][ci];
            let span = if facts.denied_capability {
                facts.segment.clone()
            } else {
                facts.clause.clone()
            };
            let Some(span) = clause_content(text, &span) else {
                continue;
            };
            let before = hits.len();
            push_norm_hit(idx, norm, src, span, hits);
            if let Some(hit) = hits.get_mut(before) {
                hit.detail = Some(detail.clone());
            }
        }
    }
}

/// The reported content of a clause or segment: from the first non-whitespace
/// byte after any leading coordinator through the last non-whitespace byte
/// that is not a delimiter. The coordinator belongs to the join, and the
/// comma, semicolon, and terminal mark belong to the sentence, so none of
/// them belong to the words a writer is being asked to change. `None` when
/// nothing but delimiters is left.
fn clause_content(text: &str, span: &Range<usize>) -> Option<Range<usize>> {
    let mut start = span.start;
    let head = &text[span.clone()];
    let first = head.trim_start();
    start += head.len() - first.len();
    if let Some(word) = first.split_whitespace().next() {
        if COORDINATORS.contains(&bare(word).as_str()) {
            let rest = &first[word.len()..];
            start += word.len() + (rest.len() - rest.trim_start().len());
        }
    }
    let end = span.start
        + text[span.clone()]
            .trim_end_matches(|c: char| {
                c.is_whitespace() || matches!(c, ',' | ';' | '.' | '!' | '?')
            })
            .len();
    (start < end).then_some(start..end)
}
