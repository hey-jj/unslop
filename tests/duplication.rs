//! SLOP-U001 self-duplication and SLOP-C009 contrast-density pins: the
//! shingle floor, the emission cap, the quotation and barrier exclusions,
//! clean-fixture silence, and the advisory instrument that never gates.

mod common;

use common::{assert_invariants, has_rule, run};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use unslop::Profile;

/// Peak-tracking wrapper around the system allocator, powering the
/// worst-case memory-budget pin below. Relaxed atomics: the counters are
/// statistics, not synchronization, and an off-by-a-few-bytes race cannot
/// move a 100 MiB assertion.
struct PeakAlloc;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn track_alloc(n: usize) {
    let cur = CURRENT.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(cur, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for PeakAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            track_alloc(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            if new_size >= layout.size() {
                track_alloc(new_size - layout.size());
            } else {
                CURRENT.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

const PARA: &str = "The gate runs the full policy over every draft before it ships to a reader.";

fn u001(report: &unslop::Report) -> Vec<&unslop::Finding> {
    report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-U001")
        .collect()
}

/// A verbatim restated paragraph (14 words, above the 10-word floor) fires
/// exactly once, on the SECOND copy, as an experimental candidate.
#[test]
fn u001_restated_paragraph_fires_once_on_the_second_copy() {
    let text = format!(
        "{PARA} Filler sentence one sits here. Filler sentence two follows with other words.\n\n{PARA}\n"
    );
    let report = run(&text, Profile::Essay);
    assert_invariants(&text, &report);
    let hits = u001(&report);
    assert_eq!(hits.len(), 1, "exactly one duplication finding");
    assert_eq!(hits[0].state, "candidate");
    assert_eq!(hits[0].lifecycle, "experimental");
    let span = &hits[0].spans[0];
    let first_end = text.find(" Filler").unwrap();
    assert!(
        span.start > first_end,
        "the reported span must be the second copy (start {} <= first copy end {first_end})",
        span.start
    );
    assert!(text[span.start..span.end].starts_with("The gate runs"));
}

/// The 10-word floor: a 9-word repeat is deliberate silence, not a miss.
#[test]
fn u001_nine_word_repeat_is_below_the_floor() {
    let nine = "The gate runs the policy over every draft twice.";
    let text = format!("{nine} Middle text sits here with several other words.\n\n{nine}\n");
    let report = run(&text, Profile::Essay);
    assert!(
        !has_rule(&report, "SLOP-U001"),
        "the floor moved: a 9-word repeat fired"
    );
}

/// A third verbatim occurrence is its own finding: one hit per repeat
/// occurrence, second and later.
#[test]
fn u001_third_occurrence_reports_again() {
    let text =
        format!("{PARA} Distinct filler follows here.\n\n{PARA} More distinct filler.\n\n{PARA}\n");
    let report = run(&text, Profile::Essay);
    assert_eq!(
        u001(&report).len(),
        2,
        "second and third copies each report"
    );
}

/// The `max_reports` cap under a degenerate repeated-stem input (the #410
/// shape): 25 disjoint repeats emit exactly the cap, never more.
#[test]
fn u001_emission_cap_is_respected() {
    let mut text = String::new();
    for i in 0..26 {
        text.push_str(&format!(
            "Verified against the frozen policy digest and the recorded manifest entry today m{i}.\n\n"
        ));
    }
    let report = run(&text, Profile::Essay);
    assert_invariants(&text, &report);
    assert_eq!(
        u001(&report).len(),
        20,
        "25 repeat occurrences must clip to max_reports"
    );
}

/// A run with a quoted copy is quotation, not self-duplication.
#[test]
fn u001_quoted_copy_is_skipped() {
    let text = format!("{PARA}\n\n> {PARA}\n");
    let report = run(&text, Profile::Essay);
    assert!(
        !has_rule(&report, "SLOP-U001"),
        "a blockquoted copy must not report as self-duplication"
    );
}

/// A quoted FIRST occurrence must not anchor the shingle index and thereby
/// suppress the prose-to-prose duplicate between the later copies:
/// `> P\n\nP\n\nP` reports exactly one finding, aligned on the third
/// (prose) copy. Before the fix the quoted copy held the bucket
/// representative, every later match paired with it, and the
/// quotation filter then discarded the lot.
#[test]
fn u001_quoted_first_occurrence_does_not_suppress_prose_duplicates() {
    let text = format!("> {PARA}\n\n{PARA}\n\n{PARA}\n");
    let report = run(&text, Profile::Essay);
    assert_invariants(&text, &report);
    let hits = u001(&report);
    assert_eq!(
        hits.len(),
        1,
        "the prose-to-prose duplicate behind a quoted first copy must report"
    );
    let span = &hits[0].spans[0];
    let third_start = text.rfind("The gate runs").unwrap();
    assert_eq!(
        span.start, third_start,
        "the reported span must be the third (prose) copy"
    );
    assert!(text[span.start..span.end].starts_with("The gate runs"));
}

/// The 0.1.6 memory pin: a near-2 MiB all-distinct-words document — every
/// 8-word shingle unique, the shape that made the old per-word String
/// tokenizer and per-shingle Vec index balloon to hundreds of MiB — must
/// analyze inside a flat heap budget. The bound covers PEAK HEAP for the
/// whole test binary (every pass, not just U001, plus this fixture's own
/// ~4 MiB): 100 MiB keeps the crate safe in a 256 MiB worker with
/// headroom, and the measured post-fix peak sits well under half the
/// bound.
#[test]
fn u001_worst_case_shape_stays_inside_the_memory_budget() {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(2 * 1024 * 1024);
    let mut word = 0usize;
    while text.len() < 1_900_000 {
        for _ in 0..15 {
            write!(text, "{word:x} ").unwrap();
            word += 1;
        }
        text.pop();
        text.push_str(".\n\n");
    }
    let config = unslop::Config::new(Profile::Essay);
    let report = unslop::analyze(text.as_bytes(), &config).unwrap();
    assert!(
        !report.findings.iter().any(|f| f.rule_id == "SLOP-U001"),
        "all-distinct words cannot duplicate"
    );
    let peak = PEAK.load(Ordering::Relaxed);
    assert!(
        peak < 100 * 1024 * 1024,
        "peak heap {peak} bytes breaches the 100 MiB worst-case budget"
    );
}

/// The prefix-decoy shape (0.1.6 defect): an earlier occurrence that
/// shares the full 8-word shingle prefix but diverges below the 10-word
/// floor must not hold the chain slot and mask the genuine duplicate
/// between the two LATER copies. Before the fix the decoy was the sole
/// representative, every later window paired only with it, and this
/// input returned no findings.
#[test]
fn u001_prefix_decoy_does_not_mask_a_later_duplicate() {
    let text = "alpha beta gamma delta epsilon zeta eta theta wrong ending. \
                amber bronze copper. \
                alpha beta gamma delta epsilon zeta eta theta iota kappa. \
                ivory jade silver. \
                alpha beta gamma delta epsilon zeta eta theta iota kappa.\n";
    let report = run(text, Profile::Essay);
    assert_invariants(text, &report);
    let hits = u001(&report);
    assert_eq!(hits.len(), 1, "the duplicate behind the decoy must report");
    let span = &hits[0].spans[0];
    assert_eq!(
        &text[span.start..span.end],
        "alpha beta gamma delta epsilon zeta eta theta iota kappa",
        "the run covers the full 10 shared words"
    );
    assert_eq!(
        span.start,
        text.rfind("alpha").unwrap(),
        "the finding sits on the third (later) copy"
    );
}

/// Maximal-run start behind a decoy: the 11-word duplicate reports its
/// FULL run from `alpha`, not one word late from `beta`. Before the fix
/// the decoy consumed the `alpha`-anchored window below the floor and the
/// reported run started at the next window over.
#[test]
fn u001_reports_the_maximal_run_start_behind_a_decoy() {
    let text = "alpha beta gamma delta epsilon zeta eta theta wrong ending. \
                amber bronze copper. \
                alpha beta gamma delta epsilon zeta eta theta iota kappa lambda. \
                ivory jade silver. \
                alpha beta gamma delta epsilon zeta eta theta iota kappa lambda.\n";
    let report = run(text, Profile::Essay);
    assert_invariants(text, &report);
    let hits = u001(&report);
    assert_eq!(hits.len(), 1, "the 11-word duplicate must report once");
    let span = &hits[0].spans[0];
    assert_eq!(
        &text[span.start..span.end],
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
        "the run must start at its true maximal start"
    );
}

/// The chain-walk worst case: a sub-floor 9-word phrase repeated thousands
/// of times across ~1.8 MiB, every occurrence sharing the shingle hash and
/// none reaching the floor, so no emission ever advances the scan past a
/// run. The walk cap must keep the pass near-linear in time and flat in
/// heap — silence is the correct verdict (9 sits below the floor), the
/// shared 100 MiB peak-heap pin applies, and the generous wall-clock
/// ceiling fails only on a quadratic blowup, not on a slow runner.
#[test]
fn u001_dense_repeated_phrase_stays_bounded() {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(2 * 1024 * 1024);
    let mut i = 0usize;
    while text.len() < 1_800_000 {
        write!(
            text,
            "alpha beta gamma delta epsilon zeta eta theta iota u{i:x}. "
        )
        .unwrap();
        i += 1;
    }
    text.push('\n');
    let config = unslop::Config::new(Profile::Essay);
    let started = std::time::Instant::now();
    let report = unslop::analyze(text.as_bytes(), &config).unwrap();
    let elapsed = started.elapsed();
    assert!(
        !report.findings.iter().any(|f| f.rule_id == "SLOP-U001"),
        "9 shared words sit below the 10-word floor"
    );
    let peak = PEAK.load(Ordering::Relaxed);
    assert!(
        peak < 100 * 1024 * 1024,
        "peak heap {peak} bytes breaches the 100 MiB dense-repeat budget"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(60),
        "dense-repeat pass took {elapsed:?}: the chain walk lost its bound"
    );
}

/// Candidate ranking happens on the TOTAL run, backward extension
/// included (the 0.1.6 Codex Finding-1 repro). The third passage matches
/// two candidates: the second passage shares 11 forward words with no
/// backward room, and the first passage shares 10 forward words plus the
/// 5 quoted words before the anchor — a 15-word total. Ranking on forward
/// length alone picked the 11-word candidate and reported a non-maximal
/// run; the maximal 15-word run must win. Extension deliberately ignores
/// quotation, so the quoted lead-in words count — only anchor windows are
/// quote-filtered.
#[test]
fn u001_ranks_candidates_by_total_run_not_forward_length() {
    let text = "> one two three four five\n\n\
                alpha beta gamma delta epsilon zeta eta theta iota kappa omega.\n\n\
                separator amber bronze copper.\n\n\
                > red blue green black white\n\n\
                alpha beta gamma delta epsilon zeta eta theta iota kappa lambda.\n\n\
                separator ivory jade silver.\n\n\
                > one two three four five\n\n\
                alpha beta gamma delta epsilon zeta eta theta iota kappa lambda.\n";
    let report = run(text, Profile::Essay);
    assert_invariants(text, &report);
    let hits = u001(&report);
    assert_eq!(hits.len(), 2, "passage-2 and passage-3 repeats each report");
    let maximal = "one two three four five\n\n\
                   alpha beta gamma delta epsilon zeta eta theta iota kappa";
    let spans: Vec<&str> = hits
        .iter()
        .map(|h| &text[h.spans[0].start..h.spans[0].end])
        .collect();
    assert!(
        spans.contains(&maximal),
        "the globally maximal 15-word run must be reported, got {spans:?}"
    );
    assert!(
        spans.contains(&"alpha beta gamma delta epsilon zeta eta theta iota kappa"),
        "the passage-2 10-word run still reports, got {spans:?}"
    );
    let max_hit = hits
        .iter()
        .find(|h| &text[h.spans[0].start..h.spans[0].end] == maximal)
        .unwrap();
    assert_eq!(
        max_hit.spans[0].start,
        text.rfind("one two").unwrap(),
        "the maximal run sits on the third (later) passage"
    );
}

/// A block code fence is a run barrier (the 0.1.6 Codex Finding-2 repro):
/// shared prose flanking DIFFERING fenced contents must not fuse into a
/// phantom run. Each visible side here sits below the 8-word shingle
/// floor, so any finding could only come from fusing across the fence —
/// the exact false duplicate the barrier prevents.
#[test]
fn u001_differing_fenced_contents_do_not_fuse_flanking_prose() {
    let text = "prefix alpha beta gamma delta epsilon\n\
                ```\n\
                excluded words live here\n\
                ```\n\
                zeta eta theta iota kappa. divider unique words.\n\n\
                prefix alpha beta gamma delta epsilon\n\
                ```\n\
                different excluded words live here\n\
                ```\n\
                zeta eta theta iota kappa.\n";
    let report = run(text, Profile::Essay);
    assert_invariants(text, &report);
    assert!(
        !has_rule(&report, "SLOP-U001"),
        "prose fused across differing fenced contents into a phantom duplicate"
    );
}

/// The chain-walk recall bound, per bucket (0.1.6 Codex Finding 3,
/// narrowed by the maximal-run polish; KNOWN-EDGES 27). One flooded
/// shingle bucket cannot mask a duplicate: 32 eight-word-aligned decoys
/// exhaust the `alpha`-anchored walk, and the run still reports through
/// the `beta`-anchored bucket — backward extension recovers `alpha`, so
/// the full 10-word run lands. The ACCEPTED residual miss floods every
/// window of the run separately (three sub-floor decoy families of 33):
/// the walk exhausts in all three buckets and the duplicate goes silent —
/// the deliberate, attacker-unrealistic recall trade behind `WALK_CAP`.
/// This half characterizes the accepted behavior without endorsing it.
#[test]
fn u001_walk_cap_recall_is_bounded_per_bucket() {
    use std::fmt::Write as _;
    let phrase = "alpha beta gamma delta epsilon zeta eta theta iota kappa";

    // Single flooded bucket: the duplicate must still report in full.
    let mut text = format!("{phrase}.\n");
    for i in 0..32 {
        writeln!(
            text,
            "alpha beta gamma delta epsilon zeta eta theta decoy{i}."
        )
        .unwrap();
    }
    writeln!(text, "{phrase}.").unwrap();
    let report = run(&text, Profile::Essay);
    assert_invariants(&text, &report);
    let hits = u001(&report);
    assert_eq!(hits.len(), 1, "one flooded bucket must not mask the run");
    let span = &hits[0].spans[0];
    assert_eq!(
        &text[span.start..span.end],
        phrase,
        "the full 10-word run reports through an unflooded window"
    );
    assert_eq!(
        span.start,
        text.rfind("alpha beta").unwrap(),
        "the finding sits on the later copy"
    );

    // Per-window flood: every bucket of the run exhausted — the accepted
    // silent miss recorded in KNOWN-EDGES 27.
    let words: Vec<&str> = phrase.split(' ').collect();
    let mut text = format!("{phrase}.\n");
    for (fam, w) in [&words[0..8], &words[1..9], &words[2..10]]
        .iter()
        .enumerate()
    {
        for i in 0..33 {
            writeln!(text, "pre{fam}x{i} {} post{fam}x{i}.", w.join(" ")).unwrap();
        }
    }
    writeln!(text, "{phrase}.").unwrap();
    let report = run(&text, Profile::Essay);
    assert!(
        !has_rule(&report, "SLOP-U001"),
        "the per-window flood is the accepted bounded-recall miss; \
         a report here means the WALK_CAP semantics moved and \
         KNOWN-EDGES 27 needs re-recording"
    );
}

/// Code regions never shingle: two identical fenced blocks are silent, and
/// prose flanking a fence does not fuse across it into a phantom run.
#[test]
fn u001_fenced_code_never_shingles() {
    let block = "use std::io; use std::fmt; use std::mem; use std::ops; use std::cmp; extra tokens here now";
    let text = format!("```\n{block}\n```\n\nProse between the fences.\n\n```\n{block}\n```\n");
    let report = run(&text, Profile::Essay);
    assert!(
        !has_rule(&report, "SLOP-U001"),
        "identical fenced blocks fired U001"
    );
}

/// Every profile applies the rule now, and repeated analysis of a
/// duplicated document is byte-identical.
#[test]
fn u001_fires_in_every_profile_and_is_deterministic() {
    let text = format!("{PARA} Filler words pad this line out considerably.\n\n{PARA}\n");
    for profile in Profile::ALL {
        let report = run(&text, profile);
        assert!(
            has_rule(&report, "SLOP-U001"),
            "U001 must fire under {}",
            profile.as_str()
        );
    }

    let dup = format!("{PARA} Filler follows in this line.\n\n{PARA}\n");
    let config = unslop::Config::new(Profile::Essay);
    let a = serde_json::to_string(&unslop::analyze(dup.as_bytes(), &config).unwrap()).unwrap();
    let b = serde_json::to_string(&unslop::analyze(dup.as_bytes(), &config).unwrap()).unwrap();
    assert_eq!(a, b, "duplication analysis must be deterministic");
}

/// Ordinary prose with no verbatim repeat stays silent.
#[test]
fn u001_is_silent_on_unrepeated_prose() {
    let text = "The rain arrived late on Thursday and stayed for two days. \
                Everyone in the valley had planned for a dry weekend, and \
                the river came up over the low road by Saturday morning.\n";
    let report = run(text, Profile::Essay);
    assert!(!has_rule(&report, "SLOP-U001"), "clean prose fired U001");
}

// --- SLOP-C009 contrast-density ---------------------------------------------

/// The instrument reports the figure and NEVER gates: an advisory
/// coverage_hint whose message carries the computed density, with the
/// result state untouched.
#[test]
fn c009_reports_the_figure_and_never_gates() {
    let t = "Findings judge house style, not authorship.\n";
    let report = run(t, Profile::Essay);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C009")
        .expect("C009 reports when the contrast family fired");
    assert_eq!(f.state, "coverage_hint");
    assert_eq!(f.lifecycle, "advisory");
    assert!(
        f.message.contains("per 1000 words"),
        "message must carry the density figure: {}",
        f.message
    );
    assert_eq!(
        report.result_state, "no_findings",
        "advisory instrument gated the run"
    );
}

/// Zero contrast hits emit nothing: an all-zero line is noise.
#[test]
fn c009_is_silent_on_a_contrast_free_document() {
    let t = "The parser handles nested lists without recursion.\n";
    let report = run(t, Profile::Essay);
    assert!(
        !has_rule(&report, "SLOP-C009"),
        "C009 emitted an all-zero line"
    );
}

/// The count covers exactly C001/C002/C003/C007/C008: a C006 balance hit
/// alone does not move the instrument.
#[test]
fn c009_excludes_concession_and_balance_shapes() {
    let t = "It is simple but powerful in daily use.\n";
    let report = run(t, Profile::Essay);
    assert!(has_rule(&report, "SLOP-C006"), "C006 control went silent");
    assert!(
        !has_rule(&report, "SLOP-C009"),
        "C009 counted a non-negation shape"
    );
}
