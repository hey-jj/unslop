//! duplication family structural rule: SLOP-U001 verbatim self-duplication.
//! Order-k word shingles seed candidate matches over the norm view; every
//! candidate is verified by exact token comparison and extended to the
//! maximal shared run, so the matcher itself cannot false-positive — every
//! emitted run is a true verbatim repeat of at least `min_run_words` words.
//! FP risk lives entirely in adjudication (deliberate refrains, legal
//! boilerplate), which the guard and judge carry.
//!
//! The shingle chain breaks at U+FFFD barriers, so code regions never
//! shingle and prose never fuses across a code span. A quote-touching
//! shingle neither anchors a bucket nor matches one: epigraphs and repeated
//! quoted claims are quotation, not self-duplication — and a quoted first
//! copy must not suppress a later prose-to-prose repeat. One forward pass,
//! exact verification bounded by the run length, a capped walk over prior
//! occurrences of each shingle (bounded recall: prefix-sharing decoys
//! cannot mask the genuine duplicate between later copies unless every
//! k-word window of the run is separately flooded past `WALK_CAP` — an
//! accepted, attacker-unrealistic edge recorded in KNOWN-EDGES), emission
//! capped at
//! `max_reports` (longest first) — near-linear time, memory proportional
//! to the token count,
//! honoring the crate-wide ban on unbounded scans. The memory bound is
//! deliberate and flat: one shared fold buffer instead of an owned String
//! per word, and an intrusive per-token chain instead of a heap Vec per
//! distinct shingle, so the worst-case 2 MiB input costs tens of megabytes,
//! not hundreds. Determinism does not depend on hash values: every hash
//! revisit is verified by text, and `DefaultHasher` is fixed-key.

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

    /// Element-wise equality of the k-word shingles at `a` and `b` — the
    /// same comparison the old `words[a..a + k] == words[b..b + k]` slice
    /// equality performed, word boundaries included.
    fn shingles_eq(&self, a: usize, b: usize, k: usize) -> bool {
        (0..k).all(|d| self.word(a + d) == self.word(b + d))
    }
}

/// Lowercased word tokens (alphanumeric plus apostrophe, with the
/// typographic apostrophe folded — the `first_token` charset from the
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
    // correctness does not depend on it — collisions are resolved by the
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

    // Shingle hash -> most-recent token index carrying that hash, with
    // earlier carriers chained through `next` (an intrusive singly linked
    // list: each token index sits in at most one bucket, so one
    // preallocated slot per token suffices). EVERY processed anchor joins
    // its chain, verified or not: keeping only one representative per
    // distinct sequence is the prefix-decoy hole — an early occurrence
    // that shares the k-word prefix but diverges below the floor would
    // hold the slot and block the genuine duplicate between later copies.
    // A revisit therefore walks the chain (most recent first, capped at
    // `WALK_CAP` entries) and keeps the candidate whose TOTAL verified
    // disjoint run — forward AND backward extension both — is maximal, so
    // occ2-vs-occ3 and occ1-vs-occ3-across-a-decoy both land and the
    // reported run is the globally maximal one among walked candidates.
    // (Ranking on forward length alone and backward-extending only the
    // winner let a candidate with a shorter forward match but a longer
    // total run lose — a non-maximal report.) The cap bounds the walk on
    // a phrase repeated N times: a candidate whose total run stays under
    // the floor costs under `floor` comparisons across both directions,
    // and a candidate at or above the floor emits and advances `i` past
    // the run, so total work stays O(WALK_CAP * tokens) — near-linear,
    // never O(N^2). The cap is also a RECALL bound, per bucket: more
    // than `WALK_CAP` occurrences of one shingle packed between a
    // genuine copy and its later repeat exhaust that bucket's walk
    // before the true partner is reached. One flooded bucket does not
    // mask a run — the run's OTHER windows sit in their own buckets, and
    // total-run ranking recovers the full extent through any unflooded
    // one (backward extension reaches the words in front of the anchor).
    // Masking a genuine duplicate therefore requires flooding EVERY
    // k-word window of the run past the cap with separate decoy
    // families — an accepted, attacker-unrealistic edge (see
    // KNOWN-EDGES): the decoy pile is itself glaring repetition on the
    // page.
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
        // it must neither anchor a bucket (a quoted FIRST copy would
        // otherwise hold the representative slot and suppress later
        // prose-to-prose repeats) nor match one (a quoted second copy is
        // not self-duplication). Windows straddling a quote boundary are
        // skipped too, so a quoted-first shape re-anchors on the first
        // all-prose window and later copies align copy-to-copy; a mixed
        // prose-and-quote duplicate still reports through its all-prose
        // windows, since run EXTENSION deliberately ignores quotation.
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
                // Walk the chain, most recent first, and keep the maximal
                // TOTAL verified run among disjoint candidates — each
                // candidate is extended in BOTH directions before ranking,
                // so a candidate with a shorter forward match but a longer
                // total run wins over a more recent, forward-longer one.
                // An entry overlapping its own revisit ("the the the") is
                // repetition inside one passage, not a duplicated passage,
                // and is skipped; a hash collision fails `shingles_eq` and
                // is skipped the same way. Ties keep the first (most
                // recent) candidate — chain order is a deterministic
                // function of the input. Cost stays bounded: a candidate
                // whose total run misses the floor stops each direction at
                // its first mismatch, under `floor` matching comparisons
                // in all, and a candidate at the floor emits and jumps.
                // (earlier start, later start, total run length in words)
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
                        // forward extension — the copies stay disjoint
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
    // decides WHICH runs survive a degenerate input, not report order.
    runs.sort_by_key(|&(e, s, len)| (std::cmp::Reverse(len), s, e));
    runs.truncate(cap);

    for (e, s, len) in runs {
        let span = toks[s].start..toks[s + len - 1].end;
        let earlier = toks[e].start..toks[e + len - 1].end;
        // Backstop only: anchor shingles are pre-filtered for quotation
        // above, so a surviving run's span cannot be fully quoted — but a
        // future anchor-filter regression must fail toward silence here,
        // not toward a quoted finding.
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
