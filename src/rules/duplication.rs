//! duplication family structural rule: SLOP-U001 verbatim self-duplication.
//! Order-k word shingles seed candidate matches over the norm view. The
//! matcher checks every candidate by exact token comparison and extends it to
//! the maximal shared run. Every emitted run is a true verbatim repeat of at
//! least `min_run_words` words, so the matcher itself cannot false-positive.
//! FP risk lives entirely in adjudication (deliberate refrains, legal
//! boilerplate), which the guard and judge carry.
//!
//! The shingle chain breaks at U+FFFD barriers. Code regions never shingle
//! and prose never fuses across a code span. A quote-touching shingle neither
//! anchors a bucket nor matches one. Epigraphs and repeated quoted claims are
//! excluded as quotation. A quoted first copy must leave later prose-to-prose
//! repeats detectable. One forward pass checks exact tokens within each run
//! length. A capped walk covers prior occurrences of each shingle.
//! Prefix-sharing decoys cannot mask a genuine duplicate between later copies
//! unless every k-word window is flooded separately past `WALK_CAP`.
//! KNOWN-EDGES records this accepted, attacker-unrealistic recall bound.
//! Emission caps at `max_reports`, longest first. Time stays near-linear and
//! memory stays proportional to token count, honoring the crate-wide ban on
//! unbounded scans. One shared fold buffer replaces an owned String per word.
//! An intrusive per-token chain replaces a heap Vec per distinct shingle. A
//! worst-case 2 MiB input uses tens of megabytes and stays below hundreds.
//! Determinism never depends on hash values: every revisit checks text and
//! `DefaultHasher` is fixed-key.

use crate::engine::{CompiledPolicy, Hit};
use crate::input::Prepared;
use crate::views::NormView;
use crate::Config;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub const HANDLED: &[&str] = &["SLOP-U001"];

/// Trigger length cap in bytes, cut on a char boundary at emit.
const TRIGGER_CAP: usize = 120;

struct Tok {
    start: usize,
    end: usize,
    /// Byte offset of this token's folded word in the shared word buffer.
    /// The word ends where the next token's word begins (buffer end for the
    /// last token): the buffer is the exact concatenation of the folded
    /// words, so no per-word length needs storing.
    word: usize,
    /// Barrier-segment id: increments at every U+FFFD. A shingle or run
    /// never spans two segments, which is what keeps prose from fusing
    /// across an excluded code region.
    seg: u32,
}

/// Tokenizer output: byte-range tokens over the norm text plus ONE shared
/// buffer holding every folded word back to back. The buffer replaces the
/// old `Vec<String>` (an owned String per word), which multiplied a 2 MiB
/// input into hundreds of megabytes of small allocations on a worst-case
/// many-short-words shape.
struct Tokens {
    toks: Vec<Tok>,
    buf: String,
}

impl Tokens {
    /// The folded word carried by token `i`.
    fn word(&self, i: usize) -> &str {
        let s = self.toks[i].word;
        let e = self
            .toks
            .get(i + 1)
            .map(|t| t.word)
            .unwrap_or(self.buf.len());
        &self.buf[s..e]
    }

    /// Element-wise equality of the k-word shingles at `a` and `b`, the
    /// same comparison the old `words[a..a + k] == words[b..b + k]` slice
    /// equality performed, word boundaries included.
    fn shingles_eq(&self, a: usize, b: usize, k: usize) -> bool {
        (0..k).all(|d| self.word(a + d) == self.word(b + d))
    }
}

/// Lowercased word tokens (alphanumeric plus apostrophe, with the
/// typographic apostrophe folded, the `first_token` charset from the
/// contrast module) with byte spans in norm coordinates.
fn tokenize(text: &str) -> Tokens {
    let mut toks = Vec::new();
    let mut buf = String::new();
    let mut seg = 0u32;
    let mut in_word = false;
    let mut start = 0usize;
    let mut word = 0usize;
    for (i, c) in text.char_indices() {
        let c = if c == '\u{2019}' { '\'' } else { c };
        if c.is_alphanumeric() || c == '\'' {
            if !in_word {
                start = i;
                word = buf.len();
                in_word = true;
            }
            for lc in c.to_lowercase() {
                buf.push(lc);
            }
        } else {
            if in_word {
                toks.push(Tok {
                    start,
                    end: i,
                    word,
                    seg,
                });
                in_word = false;
            }
            if c == '\u{FFFD}' {
                seg += 1;
            }
        }
    }
    if in_word {
        toks.push(Tok {
            start,
            end: text.len(),
            word,
            seg,
        });
    }
    Tokens { toks, buf }
}

fn shingle_hash(tokens: &Tokens, i: usize, k: usize) -> u64 {
    // Fixed-key SipHash: deterministic across runs and processes. Output
    // correctness does not depend on it, collisions are resolved by the
    // exact token comparison below.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for d in 0..k {
        tokens.word(i + d).hash(&mut h);
    }
    h.finish()
}

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-U001") else {
        return;
    };
    let rule = &cp.pkg.rules[idx];
    let k = super::param_i64(rule, "shingle_words").unwrap_or(8).max(1) as usize;
    let floor = super::param_i64(rule, "min_run_words").unwrap_or(10).max(1) as usize;
    let cap = super::param_i64(rule, "max_reports").unwrap_or(20).max(0) as usize;

    let text = norm.text.as_str();
    let src = prepared.text.as_str();
    let tokens = tokenize(text);
    let toks = &tokens.toks;
    if toks.len() < k {
        return;
    }

    // Shingle hash maps to its most recent token index. Earlier carriers
    // chain through `next`, an intrusive singly linked list. Each token sits
    // in at most one bucket, so one preallocated slot per token suffices.
    // Every processed anchor joins its chain, checked or unchecked. Keeping
    // one representative per distinct sequence opened the prefix-decoy hole:
    // an early occurrence sharing the k-word prefix but diverging below the
    // floor held the slot and blocked genuine duplicates between later
    // copies. Revisit the chain most recent first, capped at `WALK_CAP`
    // entries. Rank each candidate by total checked disjoint run after
    // forward and backward extension. This recovers both occ2-vs-occ3 and
    // occ1-vs-occ3 across a decoy. The report uses the globally maximal run
    // among walked candidates. Ranking forward length alone and
    // backward-extending only the winner let a shorter total run win and
    // produced a non-maximal report. For a phrase repeated N times, a
    // below-floor candidate uses fewer than `floor` comparisons across both
    // directions. A candidate at or above the floor emits and advances `i`
    // past the run. Total work stays O(WALK_CAP * tokens), near-linear, and
    // never becomes O(N^2). The cap also bounds recall per bucket. More than
    // `WALK_CAP` occurrences of one shingle between a genuine copy and its
    // later repeat exhaust that bucket before reaching the true partner. One
    // flooded bucket cannot mask the run. Other windows have separate
    // buckets, and any unflooded window recovers the full extent through
    // total-run ranking and backward extension. Masking the whole duplicate
    // requires flooding every k-word window past the cap with separate decoy
    // families. KNOWN-EDGES records this accepted, attacker-unrealistic edge.
    // The decoy pile itself repeats visibly.
    const WALK_CAP: usize = 32;
    const NIL: usize = usize::MAX;
    let mut heads: HashMap<u64, usize> = HashMap::new();
    let mut next: Vec<usize> = vec![NIL; toks.len()];
    // (earlier start, later start, run length in words)
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i + k <= toks.len() {
        if toks[i + k - 1].seg != toks[i].seg {
            i += 1; // shingle spans a barrier: not a unit of prose
            continue;
        }
        // A quote-touching shingle is quotation, not the writer's own prose:
        // it must neither anchor a bucket (a quoted first copy would
        // otherwise hold the representative slot and suppress later
        // prose-to-prose repeats) nor match one (a quoted second copy is
        // not self-duplication). Windows straddling a quote boundary are
        // skipped too, so a quoted-first shape re-anchors on the first
        // all-prose window and later copies align copy-to-copy. A mixed
        // prose-and-quote duplicate still reports through its all-prose
        // windows, since run EXTENSION ignores quotation.
        if norm.span_has_flag(
            &(toks[i].start..toks[i + k - 1].end),
            crate::extract::F_QUOTED,
        ) {
            i += 1;
            continue;
        }
        match heads.entry(shingle_hash(&tokens, i, k)) {
            Entry::Vacant(v) => {
                v.insert(i);
                i += 1;
            }
            Entry::Occupied(mut o) => {
                // Walk the chain most recent first and keep the maximal total
                // checked run among disjoint candidates. Extend each
                // candidate in both directions before ranking. A shorter
                // forward match with a longer total run wins over a more
                // recent, forward-longer candidate. An entry overlapping its
                // own revisit (`"the the the"`) repeats within one passage.
                // Skip it, as this rule requires a duplicated passage. The scan must
                // skip hash collisions that fail `shingles_eq`. Ties keep the
                // first, most recent candidate. Chain order is a
                // deterministic function of input. Below-floor candidates
                // stop each direction at its first mismatch, using fewer than
                // `floor` matching comparisons in all. A candidate at or
                // above the floor emits and advances the scan. The tuple
                // stores earlier start, later start, and total run length in
                // words.
                let mut best: Option<(usize, usize, usize)> = None;
                let mut e = *o.get();
                let mut walked = 0usize;
                loop {
                    if e + k <= i && tokens.shingles_eq(e, i, k) {
                        // Extend greedily to the maximal shared run,
                        // keeping the two copies disjoint (`e + len <= i`)
                        // and each side inside one barrier segment.
                        let mut len = k;
                        while i + len < toks.len()
                            && e + len < i
                            && tokens.word(e + len) == tokens.word(i + len)
                            && toks[e + len].seg == toks[e].seg
                            && toks[i + len].seg == toks[i].seg
                        {
                            len += 1;
                        }
                        // Extend backward to the true maximal start: the
                        // anchor window can sit one or more words into the
                        // real run when the run-initial window paired with
                        // a shorter decoy candidate on an earlier pass, or
                        // was skipped as quote-touching. Same guards as
                        // forward extension, the copies stay disjoint
                        // (the earlier copy's end is pinned while the
                        // later start moves left, so the gap must stay
                        // positive) and neither side crosses a barrier
                        // segment.
                        let (mut es, mut s, mut len) = (e, i, len);
                        while es > 0
                            && es + len < s
                            && tokens.word(es - 1) == tokens.word(s - 1)
                            && toks[es - 1].seg == toks[es].seg
                            && toks[s - 1].seg == toks[s].seg
                        {
                            es -= 1;
                            s -= 1;
                            len += 1;
                        }
                        if best.is_none_or(|(_, _, b)| len > b) {
                            best = Some((es, s, len));
                        }
                    }
                    walked += 1;
                    if walked >= WALK_CAP || next[e] == NIL {
                        break;
                    }
                    e = next[e];
                }
                // Prepend this anchor so LATER occurrences can pair with
                // it even when an older decoy shares the chain.
                next[i] = *o.get();
                o.insert(i);
                match best {
                    Some((e, s, len)) if len >= floor => {
                        runs.push((e, s, len));
                        // Advance past the repeated run: sub-runs of an
                        // emitted run are not separate findings. `s + len`
                        // is the anchor plus the winner's forward-extended
                        // length, so progress is at least `k` words.
                        i = s + len;
                    }
                    _ => i += 1,
                }
            }
        }
    }

    // Longest first under the emission cap, position as the deterministic
    // tiebreak. `assemble` re-sorts findings by span, so the cap order only
    // selects the runs that survive a degenerate input. Report order follows
    // the span sort.
    runs.sort_by_key(|&(e, s, len)| (std::cmp::Reverse(len), s, e));
    runs.truncate(cap);

    for (e, s, len) in runs {
        let span = toks[s].start..toks[s + len - 1].end;
        let earlier = toks[e].start..toks[e + len - 1].end;
        // Backstop only: anchor shingles are pre-filtered for quotation
        // above, so a surviving run's span cannot be fully quoted, but this
        // check must suppress a quoted run if the anchor filter regresses.
        if norm.all_quoted(&span) || norm.all_quoted(&earlier) {
            continue;
        }
        // Map exactly as the contrast module does: through the segment
        // table, widened against the source. Trigger fidelity re-verifies
        // the reported slice at emit, so a mapping bug fails closed.
        let Some(source_span) = norm.to_source(span.clone()) else {
            continue;
        };
        let source_span = crate::widen_to_char_boundaries(src, source_span);
        if source_span.start >= source_span.end {
            continue;
        }
        let mut hit = Hit::new(idx, source_span);
        let mut trigger = &text[span];
        if trigger.len() > TRIGGER_CAP {
            let mut cut = TRIGGER_CAP;
            while !trigger.is_char_boundary(cut) {
                cut -= 1;
            }
            trigger = &trigger[..cut];
        }
        hit.trigger = Some(trigger.to_string());
        hits.push(hit);
    }
}
