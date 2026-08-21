//! Calibration coverage against real-world markdown: table-cell
//! barriers — no match may fuse across the `|` cell delimiter; A002
//! `harness` narrowed to the verb-with-object slop
//! form; and mention-vs-use — the code-span authoring convention for
//! quoted banned-word lists, with the plain-prose enumeration residual pinned.

mod common;

use common::{assert_invariants, has_rule, run};
use unslop::Profile;

// --- Table cells scanned as prose must not fuse across cell delimiters ------

/// S001 (`^--\s{1,8}\S`) and M001 (`\s--\s`) can otherwise fire on table
/// placeholder-dash cells by pairing one cell's `--` with the NEXT cell's
/// text across the Block newline. The cell-end barrier must stop both.
#[test]
fn placeholder_dash_cells_no_longer_fuse_across_cell_boundaries() {
    let text = "# Audit\n\n\
        | Crate | Verdict | Notes |\n\
        | --- | --- | --- |\n\
        | serde | -- | not audited |\n\
        | tokio | -- | pending |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(
        !has_rule(&report, "SLOP-S001"),
        "S001 fused across cells: {:?}",
        common::rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-M001"),
        "M001 fused across cells: {:?}",
        common::rule_ids(&report)
    );
}

/// Genuine slop INSIDE one cell must still fire: the barrier sits at the cell
/// end only, never inside it, and cell interiors remain scanned prose.
#[test]
fn genuine_slop_inside_a_single_cell_still_fires() {
    let text = "| Item | Note |\n\
        | --- | --- |\n\
        | widget | a truly game-changer design |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    let a001: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A001")
        .collect();
    assert_eq!(a001.len(), 1, "in-cell lexicon hit fires");
    let span = &a001[0].spans[0];
    assert_eq!(&text[span.start..span.end], "game-changer");
}

/// A signature line inside one cell is in-cell content, not cross-cell
/// fusion: the block-start position of the cell's own text must survive the
/// barrier (which is why the barrier is at the cell END, not the start).
/// The dash-opened signature line belongs to SLOP-S004 since the split.
#[test]
fn signature_shape_within_one_cell_still_fires() {
    let text = "| Item | Note |\n\
        | --- | --- |\n\
        | -- Claude | sig in cell |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    let s004: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-S004")
        .collect();
    assert_eq!(s004.len(), 1, "in-cell signature fires");
    let span = &s004[0].spans[0];
    assert!(text[span.start..span.end].starts_with("-- C"));
}

/// The table's leading edge: a paragraph ending `--` directly before a table
/// must not pair with the first header cell into S001's shape.
#[test]
fn prose_before_a_table_does_not_fuse_into_the_first_cell() {
    let text = "--\n\n| Alpha | Beta |\n| --- | --- |\n| a | b |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(
        !has_rule(&report, "SLOP-S004"),
        "S004 fused prose into the table head: {:?}",
        common::rule_ids(&report)
    );
}

/// A normal prose document is untouched by the table barriers: the real
/// signature shape still fires, and an ordinary paragraph raises nothing new.
#[test]
fn normal_prose_is_unaffected_by_table_barriers() {
    let text = "The parser handles nested lists.\n\n-- Claude\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(
        has_rule(&report, "SLOP-S004"),
        "prose signature still fires"
    );

    let clean = "The parser handles nested lists without recursion.\n";
    let report = run(clean, Profile::GeneralWriting);
    assert_invariants(clean, &report);
    assert!(
        report.findings.iter().all(|f| f.state != "violation"),
        "clean prose stays clean: {:?}",
        common::rule_ids(&report)
    );
}

// --- A002 `harness` narrowed to the verb-with-object slop form --------------

fn a002_fires(text: &str) -> bool {
    run(text, Profile::Doc)
        .findings
        .iter()
        .any(|f| f.rule_id == "SLOP-A002" && f.state == "violation")
}

/// FN regression: a determiner-only form silently
/// misses determiner-less verb slop. The exact verified-FN repros — each
/// clean under a determiner-only calibration — must fire.
#[test]
fn a002_determiner_less_verb_slop_fires() {
    for text in [
        // The three verified-FN repros, isolated and re-verified.
        "You can harness machine learning without extra setup.",
        "The SDK lets you harness modern APIs with one call.",
        "Use it to harness advanced language models in CI.",
        // Determiner-less variants: imperative start, AI-domain objects,
        // plural-subject + AI object, modal and helper-verb signals.
        "Harness modern tooling in one step.",
        "Developers harness AI daily.",
        "Teams can harness LLMs for code review.",
        "It helps harness generative AI safely.",
        "Let's harness large language models today.",
        "We will harness neural networks here.",
    ] {
        assert!(a002_fires(text), "determiner-less verb slop missed: {text}");
    }
}

/// NOUN uses of `harness` dominate real technical prose and must pass. The
/// rule requires the slop VERB construction — determiner+object, a preceding
/// verb/subject signal, the sentence-start imperative, an AI-domain object,
/// or the `harnessing` gerund; every noun use passes structurally.
#[test]
fn a002_harness_verb_with_object_fires() {
    for text in [
        "You can harness the power of X here.",
        "It harnesses the capabilities of the runtime.",
        "This lets you harness its potential.",
        "We harness your existing pipeline.",
        "Harnessing its potential is straightforward.",
        "By harnessing the capabilities of the compiler, it checks more.",
    ] {
        assert!(a002_fires(text), "verb-form slop did not fire: {text}");
    }
}

#[test]
fn a002_harness_noun_uses_do_not_fire() {
    for text in [
        "The test harness runs nightly.",
        "The orchestration harness deploys the fleet.",
        "The CI harness caches builds between runs.",
        "The harness ran without failures.",
        "Each harness writes its logs to disk.",
        "We added three harnesses for the parser.",
    ] {
        assert!(!a002_fires(text), "noun use fired: {text}");
    }
}

/// Every determiner-less/other-determiner verb
/// form confirmed as a silent FN must fire — in the imperative
/// (standalone) carrier via the sentence-start form AND in a signaled
/// carrier via the verb-context alternation.
#[test]
fn a002_confirmed_verb_forms_fire_in_both_carriers() {
    for phrase in [
        "harness machine learning",
        "harness advanced models",
        "harness data at scale",
        "harness LLMs",
        "harness real-time data",
        "harness such power",
        "harness all the power",
        "harness a modern API",
        "harness an advanced model",
        "harness some existing data",
        "harness that capability",
        "harness those models",
    ] {
        let standalone = format!("Harness{} today.", &phrase[7..]);
        assert!(
            a002_fires(&standalone),
            "imperative carrier missed: {standalone}"
        );
        let signaled = format!("You can {phrase} today.");
        assert!(a002_fires(&signaled), "signaled carrier missed: {signaled}");
    }
}

/// The decisive pair: `these` fired while `those` shipped clean, purely
/// from a half-covered demonstrative set. The closed
/// 4-member paradigm (this/that/these/those) makes the pair behave
/// IDENTICALLY in the unsignaled third-person carrier neither verb signal
/// nor imperative reaches.
#[test]
fn a002_demonstrative_paradigm_is_symmetric() {
    for det in ["this", "that", "these", "those"] {
        let text = format!("The platform harnesses {det} model daily.");
        assert!(a002_fires(&text), "demonstrative asymmetry: {text}");
    }
}

/// Boundary pins for the harness calibration, so neither residual is
/// accidental. Over-fire side (accepted, FN-safety first): a sentence-start
/// noun compound matches the imperative form and FIRES — a documented FP,
/// waivable, never a miss. Miss side (the documented residual): a bare
/// plural-noun subject with a base verb and a non-AI object is structurally
/// identical to a noun compound ("harness telemetry") and is not matched.
#[test]
fn a002_harness_calibration_boundaries_are_pinned() {
    assert!(
        a002_fires("Harness configuration lives in rig.toml."),
        "sentence-start over-fire is the accepted side of the boundary"
    );
    // Covering "harness that capability" costs the relativizer
    // over-fire — accepted, documented, waivable, never a miss.
    assert!(
        a002_fires("We ship a harness that runs nightly."),
        "relativizer over-fire is the accepted side of the boundary"
    );
    assert!(
        !a002_fires("Teams harness telemetry pipelines in production."),
        "documented residual (b) changed shape"
    );
    // Residual (a): unsignaled third-person subject + article object.
    assert!(
        !a002_fires("The platform harnesses a modern API under the hood."),
        "documented residual (a) changed shape"
    );
}

// --- C-family vs the cell barrier, both sides pinned -----------------------

/// C006's \s{1,8} gap cannot cross the cell barrier, so
/// the cross-cell contrast is SUPPRESSED (working-as-designed —
/// attacker-unrealistic as organic slop, candidate tier even in-cell), while
/// the same phrase inside one cell still fires.
#[test]
fn c006_cross_cell_suppressed_but_single_cell_fires() {
    let cross = "| A | B |\n| --- | --- |\n| simple | but flexible |\n";
    let report = run(cross, Profile::GeneralWriting);
    assert_invariants(cross, &report);
    assert!(
        !has_rule(&report, "SLOP-C006"),
        "cross-cell C006 should be suppressed by the barrier: {:?}",
        common::rule_ids(&report)
    );
    let single = "| A | B |\n| --- | --- |\n| x | simple but flexible |\n";
    let report = run(single, Profile::GeneralWriting);
    assert_invariants(single, &report);
    assert!(
        has_rule(&report, "SLOP-C006"),
        "in-cell C006 must still fire: {:?}",
        common::rule_ids(&report)
    );
}

/// C005's [^.!?] classes admit U+FFFD and the newline, so
/// the cross-cell tricolon BRIDGE persists — candidate tier, surfaced, never
/// silence. Pinned so the asymmetry with C006 stays deliberate; excluding
/// U+FFFD from the C-family classes would silence contrast-slop legitimately
/// spanning an inline-code barrier (a real FN) and must not be done casually.
#[test]
fn c005_cross_cell_bridge_persists_as_candidate() {
    let text = "| A | B | C |\n| --- | --- | --- |\n\
        | fast | Linux, macOS, and Windows | reliable |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    let c005: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C005")
        .collect();
    assert!(
        !c005.is_empty(),
        "C005 bridge disappeared — if intentional, re-document the accepted edge"
    );
    for f in &c005 {
        assert_eq!(f.state, "candidate", "C005 must stay candidate tier");
    }
}

/// The other three homographs are untouched by the harness narrowing.
#[test]
fn a002_other_homographs_still_fire_in_bare_prose() {
    for text in [
        "This opens a new realm of possibilities.",
        "Users navigate complexity with ease.",
        "The testing landscape keeps changing.",
    ] {
        assert!(a002_fires(text), "homograph did not fire: {text}");
    }
}

// --- Mention-vs-use on quoted banned-word lists -----------------------------
//
// Decision: NO prose-list downgrade. The FN-safe authoring
// conventions are code spans / fenced code (excluded by segmentation) and
// blockquotes (deterministic candidate downgrade with provenance). A
// downgrade keyed on "list items under an avoid/banned heading" was rejected:
// LLMs produce "avoid"-headed lists organically, and any genuine slop
// sentence can be authored as a list item under one — a silent-FN channel.

/// The convention works: a style guide quoting every banned term in code
/// spans and a fenced block carries no ornamental/filler finding at all.
#[test]
fn banned_words_in_code_spans_and_fences_do_not_fire() {
    let text = "# Style guide\n\n\
        Never use these words: `delve`, `game-changer`, `robust`, `essentially`.\n\n\
        The banned list in fenced form:\n\n\
        ```text\ndelve\ngame-changer\nessentially\n```\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.rule_id == "SLOP-A001" || f.rule_id == "SLOP-T001"),
        "quoted-in-code banned words fired: {:?}",
        common::rule_ids(&report)
    );
    assert!(
        report.findings.iter().all(|f| f.state != "violation"),
        "style guide carries violations: {:?}",
        common::rule_ids(&report)
    );
}

/// The blockquote convention: quoted banned words downgrade to candidate
/// with claimed-quotation provenance — surfaced, never silent, not blocking.
#[test]
fn banned_words_in_a_blockquote_downgrade_to_candidate() {
    let text = "# Style guide\n\n> Avoid: delve, game-changer.\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    let a001: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A001")
        .collect();
    assert_eq!(a001.len(), 2);
    for f in &a001 {
        assert_eq!(f.state, "candidate", "quotation downgrade applies");
        assert_eq!(f.provenance, "claimed-quotation");
    }
}

/// Residual pin: PLAIN-PROSE enumeration of banned words still fires as a
/// violation. Deliberate — see the module comment above.
/// If this test ever goes red because someone added a prose-list downgrade,
/// that change must first prove it cannot hide genuine slop.
#[test]
fn plain_prose_banned_word_enumeration_still_fires() {
    let text = "# Style guide\n\nWords to avoid:\n\n- delve\n- game-changer\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    let a001: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A001" && f.state == "violation")
        .collect();
    assert_eq!(a001.len(), 2, "plain-prose mentions stay violations");
}

/// The guardrail the rejected downgrade was measured against: genuine slop in
/// ordinary prose — including inside a list under an "avoid" heading — fires.
#[test]
fn genuine_slop_in_prose_and_avoid_lists_still_fires() {
    let text = "We delve into the internals of the parser.\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(has_rule(&report, "SLOP-A001"));

    let text = "Mistakes to avoid:\n\n- Forgetting to delve into the config first.\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "SLOP-A001" && f.state == "violation"),
        "slop sentence under an avoid heading must still fire"
    );
}

/// Diagnosis pin (A001 on crate names in audit tables): a
/// single-cell
/// lexicon word, a crate NAMED `robust` or `Vibrant`, is NOT cross-cell
/// fusion and deliberately still fires. Distinguishing a name column from a
/// description cell is not decidable mechanically, and a cells-are-data
/// downgrade would hide genuine slop written in a description cell — a
/// silent-FN channel. The authoring convention is code spans: `robust` in
/// backticks is excluded by segmentation (see the mention-vs-use tests). Pinned so the
/// residual is visible, not accidental.
#[test]
fn single_cell_lexicon_word_is_a_documented_residual_not_fusion() {
    let text = "| Crate | Verdict |\n\
        | --- | --- |\n\
        | robust | clean |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    // robust sits on SLOP-A010 since the ornamental set split by whether a
    // word still has a plain sense to mean.
    let hits: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A010")
        .collect();
    assert_eq!(hits.len(), 1, "single-cell lexicon word still fires");
    let span = &hits[0].spans[0];
    assert_eq!(&text[span.start..span.end], "robust");

    // The convention: the same table with the crate name in a code span is
    // clean — the code-span exclusion plus its barrier cover it.
    let text = "| Crate | Verdict |\n\
        | --- | --- |\n\
        | `robust` | clean |\n";
    let report = run(text, Profile::GeneralWriting);
    assert_invariants(text, &report);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "code-span crate name must not fire: {:?}",
        common::rule_ids(&report)
    );
}

// --- SLOP-C007 apophatic self-definition: adjudicated boundary pins ---------
//
// The spec's 16 positive and 16 negative boundary examples, pinned so the T1
// suppression classifier and the T2-T4 trigger regexes survive future tuning.
// Positives are third-person self-description; negatives are imperatives,
// second-person directives, parenthetical interpolations, and shapes owned by
// SLOP-C001/C003. The deny-list homograph FN is pinned separately below.

const C007_POSITIVES: &[&str] = &[
    "Findings judge house style, not authorship.",
    "The report carries evidence, not verdicts.",
    "This tool is a linter, not a detector.",
    "It measures diction, not intent.",
    "The digest identifies the policy, not the tarball.",
    "Errors are surfaced, never swallowed.",
    "The cache is an optimization, not a source of truth.",
    "This limit is a floor, not a ceiling.",
    "The skill gates drafts, not people.",
    "The check enforces style, not correctness.",
    "The goal is clarity, not coverage.",
    "Waivers document exceptions, not permissions.",
    "The check isn't about speed, it's about correctness.",
    "Configuration is not a convenience but a contract.",
    "The scanner is not a formatter. It is a gate.",
    "Profiles describe the writing, not the author.",
];

const C007_NEGATIVES: &[&str] = &[
    "Use tabs, not spaces.",
    "Never commit secrets, not even in fixtures.",
    "Do not retry on 4xx.",
    "You cannot call this from a signal handler, not even with a lock held.",
    "Prefer &str, not String, in argument position.",
    "Pass --force, not -f, to override.",
    "Rust, not C, was chosen for the rewrite.",
    "The tests cover ASCII but not UTF-16.",
    "If not set, the default applies.",
    "Whether or not the flag is present, parsing proceeds.",
    "The parser accepts CRLF, not because it is valid, but because real files contain it.",
    "Use exponential backoff rather than fixed sleeps.",
    "When in doubt, use the builder, not the raw constructor.",
    "404 Not Found is returned for missing keys.",
    "> \"It's not a bug, it's a feature.\"",
];

#[test]
fn c007_positive_boundaries_fire_as_experimental_candidates() {
    for text in C007_POSITIVES {
        let t = format!("{text}\n");
        let report = run(&t, Profile::Doc);
        assert_invariants(&t, &report);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-C007")
            .unwrap_or_else(|| panic!("C007 silent on positive {text:?}"));
        assert_eq!(f.state, "candidate", "{text:?}");
        assert_eq!(
            f.lifecycle, "experimental",
            "{text:?}: experimental lifecycle reports without gating"
        );
    }
}

#[test]
fn c007_negative_boundaries_stay_silent() {
    for text in C007_NEGATIVES {
        let t = format!("{text}\n");
        let report = run(&t, Profile::Doc);
        assert_invariants(&t, &report);
        assert!(
            !has_rule(&report, "SLOP-C007"),
            "C007 fired on negative {text:?}: {:?}",
            common::rule_ids(&report)
        );
    }
}

/// A tail whose negation runs straight into an -ing word is a manner clause,
/// not a noun phrase, so the T1 trigger never opens on it. The test is
/// adjacency: a determiner between the two puts a real noun back in the tail.
/// Five words end in -ing without being participles and are denied on the way
/// through, which is what keeps `not everything` and `not during matching`
/// firing exactly as they did.
#[test]
fn c007_reads_a_participial_tail_as_a_manner_clause() {
    for text in [
        "She listened, never judging anyone.",
        "He worked all night, never complaining.",
        "They shipped it quietly, not making a fuss.",
    ] {
        let t = format!("{text}\n");
        let report = run(&t, Profile::Doc);
        assert_invariants(&t, &report);
        assert!(
            !has_rule(&report, "SLOP-C007"),
            "C007 read a participial adjunct as a tail: {text:?}"
        );
    }
    for text in [
        "The rule reports the span, not the sentence.",
        // everything is on the deny-list.
        "It flags the shape, not everything.",
        // A determiner between the negation and the -ing word.
        "It flags the shape, not the beginning.",
        // during is denied so the participle test reads correctly.
        "predicates are cloned during build, not during matching.",
    ] {
        let t = format!("{text}\n");
        let report = run(&t, Profile::Doc);
        assert_invariants(&t, &report);
        assert!(
            has_rule(&report, "SLOP-C007"),
            "C007 lost a real tail: {text:?}"
        );
    }
}

/// Degenerate-tail calibration: an empty or WHITESPACE-ONLY span between
/// the keyword and the terminal is not a noun phrase, so the T1 parser must
/// stay silent on it. The long-run variant (more whitespace than the 8-char
/// keyword-boundary skip consumes) is the regression pin: before the
/// content check it parsed the residual spaces as an "NP" and fired with a
/// snippet like `, not         .`. A real NP still fires as candidate.
#[test]
fn c007_whitespace_only_np_is_silent() {
    for text in [
        "Findings, not   .\n",              // short run: consumed by the skip
        "Findings, not            .\n",     // long run: residual ws is the "NP"
        "Findings, never \t \t    \t  .\n", // mixed space/tab, `never` keyword
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-C007"),
            "C007 fired on whitespace-only NP {text:?}: {:?}",
            common::rule_ids(&report)
        );
    }

    // Control: the canonical specimen with a real NP still fires.
    let t = "Findings judge house style, not authorship.\n";
    let report = run(t, Profile::Doc);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C007")
        .expect("real NP must still fire");
    assert_eq!(f.state, "candidate");
}

/// ACCEPTED FALSE NEGATIVE (KNOWN-EDGES): C007 tail matching is
/// ASCII-whitespace-only. A non-ASCII space (here U+00A0 NBSP) between the
/// keyword and the noun phrase fails the keyword's right-boundary check, so
/// the tail does not fire. Accepted as attacker-unrealistic; this test
/// characterizes the behavior, it does not endorse widening the match.
#[test]
fn c007_nonascii_space_tail_is_an_accepted_false_negative() {
    let t = "Findings judge house style, not\u{00A0}authorship.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "the NBSP accepted-FN pin moved: {:?}",
        common::rule_ids(&report)
    );
}

/// The documented deny-list false negative: `Set` is the noun/verb homograph
/// on the imperative opener list, so this descriptive sentence is wrongly
/// suppressed. Accepted by design — the classifier's bias is FP-safety, and
/// every suppression doubt resolves toward silence.
#[test]
fn c007_denylist_homograph_is_an_accepted_false_negative() {
    let t = "Set operations return unions, not lists.\n";
    let report = run(t, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "the deny-list FN pin moved: {:?}",
        common::rule_ids(&report)
    );
}

/// The subject-elided contrast reports a candidate in every profile: no
/// profile carries a legitimate operand-contrast case the mood classifier
/// cannot reach.
#[test]
fn c007_reports_a_candidate_in_every_profile() {
    let t = "Returns a reference, not a copy.\n";
    for profile in Profile::ALL {
        let report = run(t, profile);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-C007")
            .unwrap_or_else(|| panic!("C007 silent in {}", profile.as_str()));
        assert_eq!(f.state, "candidate", "in {}", profile.as_str());
        assert_eq!(f.lifecycle, "experimental", "in {}", profile.as_str());
    }
}

/// Span and trigger fidelity for the T1 evaluator: the reported source slice
/// is exactly the comma-not tail of the canonical specimen.
#[test]
fn c007_canonical_specimen_span_is_the_tail() {
    let t = "Findings judge house style, not authorship.\n";
    let report = run(t, Profile::Doc);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C007")
        .expect("canonical specimen fires");
    let span = &f.spans[0];
    assert_eq!(&t[span.start..span.end], ", not authorship.");
    assert_eq!(common::snippet(f), ", not authorship.");
}

/// T1 sites inside code formatting are mentions, never prose: the engine's
/// segmentation must keep the canonical specimen silent when fenced.
#[test]
fn c007_quoted_in_code_never_fires() {
    let t = "Prose line.\n\n```\nFindings judge house style, not authorship.\n```\n";
    let report = run(t, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "C007 fired from inside a code fence"
    );
}

// --- 0.1.6 abbreviation-period fix: the R6 false-split class ----------------
//
// Before the fix, any `.` inside the NP scan closed the tail, so `U.S.`
// produced the truncated false candidate `, not in the U.`, and the clause
// walk-back treated an abbreviation period as a clause boundary, shortening
// the clause the suppression classifier sees. `period_is_terminal` bounds
// both: a period followed by an alphanumeric or by whitespace plus a
// lowercase continuation is sentence-internal.

fn c007_findings(report: &unslop::Report) -> Vec<&unslop::Finding> {
    report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C007")
        .collect()
}

/// The exact R6 repro: the abbreviation-internal period plus the lowercase
/// continuation (`U.S. but`) must not manufacture a candidate. The real tail
/// scan then dies at the comma after `Asia`, so the document is C007-silent.
#[test]
fn c007_abbreviation_mid_tail_no_longer_false_fires() {
    let t = "Adoption is concentrated, not in the U.S. but in Asia, where usage doubled.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "the U.S. false split is back: {:?}",
        common::rule_ids(&report)
    );
}

/// The R6 acceptance case proper — NO trailing comma, so the tail parser
/// reaches the sentence terminal and the comma variant above cannot stand in
/// for it. A `not X but Y` locative/technical contrast is a legitimate
/// contrast (same class as the design's `Returns a reference, not a copy.`
/// keep-rule): the contrastive `but` continuation inside the tail is
/// SLOP-C008's pair territory, never a C007 apophatic caveat. A C008
/// candidate here would be adjudicable; a C007 false positive is not.
#[test]
fn c007_not_x_but_y_contrast_without_trailing_comma_stays_silent() {
    let t = "Adoption is concentrated, not in the U.S. but in Asia.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "C007 fired on the not-X-but-Y contrast (no trailing comma): {:?}",
        common::rule_ids(&report)
    );

    // Controls: the bare apophatic tail still fires, and a genuine tail
    // ENDING in the same abbreviation still fires with its full span.
    let t = "Findings judge house style, not authorship.\n";
    let report = run(t, Profile::Doc);
    assert_eq!(
        c007_findings(&report).len(),
        1,
        "the canonical specimen went silent"
    );
    let t = "The survey covers Europe, not the U.S.\n";
    let report = run(t, Profile::Doc);
    let f = c007_findings(&report);
    assert_eq!(f.len(), 1, "the abbreviation-final tail went silent");
    let span = &f[0].spans[0];
    assert_eq!(&t[span.start..span.end], ", not the U.S.");
}

/// A genuine tail ENDING in an abbreviation fires with the full span, where
/// it used to truncate at the abbreviation's first period (`, not the U.`).
#[test]
fn c007_tail_ending_in_abbreviation_fires_with_the_full_span() {
    let t = "The survey covers Europe, not the U.S.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    let f = c007_findings(&report);
    assert_eq!(f.len(), 1, "genuine abbreviation tail must fire once");
    let span = &f[0].spans[0];
    assert_eq!(&t[span.start..span.end], ", not the U.S.");
}

/// The clause_start half of the fix, pinned so it cannot silently revert:
/// the walk-back must cross `U.S.` and recover the WHOLE clause, whose
/// imperative opener (`Use`) then suppresses the tail. Under the old
/// walk-back the abbreviation period read as a clause boundary, the
/// recovered clause was just `hosted mirror`, the opener was invisible, and
/// this sentence false-fired — so this assertion fails if the
/// `period_is_terminal` arm is removed from `clause_start`. The
/// non-directive control pins that crossing the abbreviation did not also
/// change the verdict on a clause that should fire.
#[test]
fn c007_clause_walkback_crosses_abbreviation() {
    let t = "Use the U.S. hosted mirror, not a pilot.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "the walk-back stopped at the abbreviation and lost the opener: {:?}",
        common::rule_ids(&report)
    );

    let t = "The rollout targets the U.S. market, not a pilot.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    let f = c007_findings(&report);
    assert_eq!(f.len(), 1);
    let span = &f[0].spans[0];
    assert_eq!(&t[span.start..span.end], ", not a pilot.");
}

/// Guard for the fix eating the genuine end-of-text terminal: the canonical
/// specimen still fires (its terminal period is followed by a newline).
#[test]
fn c007_genuine_terminal_still_fires_after_the_fix() {
    let t = "Findings judge house style, not authorship.\n";
    let report = run(t, Profile::Doc);
    let f = c007_findings(&report);
    assert_eq!(f.len(), 1, "the canonical specimen went silent");
    assert_eq!(f[0].state, "candidate");
}

/// Mid-NP `e.g.` handled means CORRECT SPAN, not silence: both abbreviation
/// periods read as NP content and the tail closes at the true terminal. The
/// carrier avoids an imperative opener (the design's `Use the alias, ...`
/// specimen is suppressed by the opener deny-list, so it cannot pin the NP
/// behavior), and the span-fidelity assertion covers the case that used to
/// truncate.
#[test]
fn c007_eg_mid_np_fires_with_the_complete_span() {
    let t = "The docs cite the alias, not e.g. the raw path.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    let f = c007_findings(&report);
    assert_eq!(f.len(), 1);
    let span = &f[0].spans[0];
    assert_eq!(&t[span.start..span.end], ", not e.g. the raw path.");
    assert_eq!(common::snippet(f[0]), ", not e.g. the raw path.");
}

/// ACCEPTED EDGE (KNOWN-EDGES): lowercase sentence starts. Chat-style
/// prose that opens its next sentence lowercase reads the real terminal as
/// a continuation. Two observable shapes, both characterized here: when the
/// continuation reaches another terminal inside the NP budget the tail
/// fires with an over-wide span (still genuine slop, still verified by
/// trigger fidelity); when the continuation meets an excluded character
/// (`,` `;` `:` newline) or exhausts `tail_np_max_bytes` first, the tail is
/// an accepted false negative — the fail-toward-silence trade.
#[test]
fn c007_lowercase_next_sentence_is_an_accepted_edge() {
    // Silent shape: the comma in the continuation kills the candidate.
    let t = "Findings judge house style, not authorship. they never block, ever.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-C007"),
        "the lowercase-continuation accepted-FN pin moved: {:?}",
        common::rule_ids(&report)
    );

    // Over-wide shape: the continuation ends at the next terminal, so the
    // genuine tail fires with the extended span.
    let t = "Findings judge house style, not authorship. they never block.\n";
    let report = run(t, Profile::Doc);
    let f = c007_findings(&report);
    assert_eq!(f.len(), 1);
    let span = &f[0].spans[0];
    assert_eq!(
        &t[span.start..span.end],
        ", not authorship. they never block."
    );
}

// --- SLOP-A005 metaphor-reach phrases ---------------------------------------

/// The five adjudicated corpus-A catches, pinned as experimental candidates
/// on a hot profile.
#[test]
fn a005_adjudicated_positives_fire_candidate_on_readme() {
    for text in [
        "The data tells a more textured story about adoption.\n",
        "This result is worth sitting with.\n",
        "The invitation is to rethink the pipeline.\n",
        "The metric serves as a canary for regressions.\n",
        "It weaves together three subsystems.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-A005")
            .unwrap_or_else(|| panic!("A005 silent on {text:?}"));
        assert_eq!(f.state, "candidate", "{text:?}");
        assert_eq!(f.lifecycle, "experimental", "{text:?}");
    }
}

/// The closed-boundary hazard pin (`\bcompass` reached into `compassion` in
/// the probe's loose form) plus the no-idiom-shape control.
#[test]
fn a005_closed_boundaries_and_plain_prose_stay_silent() {
    for text in [
        "Their approach serves as a compassionate model for teams.\n",
        "She wrote a story about the outage.\n",
        "The mine's canary protocol is documented separately.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            !has_rule(&report, "SLOP-A005"),
            "A005 fired on negative {text:?}"
        );
    }
}

/// quotation_suppress: an idiom inside a claimed-quotation region is
/// DROPPED, not downgraded — the quoted author's diction is not the
/// writer's. The adjacent prose control proves the rule itself is hot.
#[test]
fn a005_quoted_hits_are_suppressed_entirely() {
    let t = "Prose line.\n\n> The paper weaves together two traditions of analysis.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-A005"),
        "quoted A005 hit survived suppression: {:?}",
        common::rule_ids(&report)
    );

    let t = "It weaves together two traditions of analysis.\n";
    let report = run(t, Profile::Doc);
    assert!(has_rule(&report, "SLOP-A005"), "prose control went silent");
}

/// quotation_suppress across a WRAPPED blockquote: the idiom spans the
/// softbreak between two `> ` lines, and the Break op's norm segment must
/// carry the quoted flag or `all_quoted` breaks at the wrap and the quoted
/// hit leaks through. The soft-wrapped PROSE control pins that the phrase
/// still assembles and fires across an unquoted softbreak.
#[test]
fn a005_wrapped_blockquote_hits_are_suppressed() {
    let t = "> The data tells a more\n> textured story.\n";
    let report = run(t, Profile::Doc);
    assert_invariants(t, &report);
    assert!(
        !has_rule(&report, "SLOP-A005"),
        "A005 survived quotation_suppress across the blockquote wrap: {:?}",
        common::rule_ids(&report)
    );

    let t = "The data tells a more\ntextured story.\n";
    let report = run(t, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A005"),
        "soft-wrapped prose control went silent"
    );
}

/// Every profile applies the rule: the metaphor-reach idioms carry no
/// term-of-art sense in the writing this package reads.
#[test]
fn a005_applies_in_every_profile() {
    for profile in Profile::ALL {
        let report = run("It weaves together three subsystems.\n", profile);
        assert!(
            has_rule(&report, "SLOP-A005"),
            "A005 must fire under {}",
            profile.as_str()
        );
    }
}

// --- SLOP-V004 agent-loop vocabulary ----------------------------------------

/// Every lexicon phrase fires in a durable-profile document, and the two
/// construction-shaped specimens from the eval evidence fire with them.
#[test]
fn v004_lexicon_and_construction_positives_fire() {
    let pkg = unslop::policy::load().unwrap();
    let rule = pkg.rule_by_id("SLOP-V004").unwrap();
    for term in &rule.terms {
        let text = format!("The check was rerun {term} without changes.\n");
        let report = run(&text, Profile::GeneralWriting);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-V004")
            .unwrap_or_else(|| panic!("V004 silent on lexicon term {term:?}"));
        assert_eq!(f.state, "candidate", "{term:?}");
    }
    for text in [
        "Flagged for JJ: the digest moved between runs.\n",
        "All three figures confirmed.\n",
        "All 12 counts verified against the ledger.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule_id == "SLOP-V004" && f.state == "candidate"),
            "V004 construction silent on {text:?}"
        );
    }
}

/// The case-sensitivity bound on the Flagged-for construction: lowercase
/// mid-sentence process prose never fires.
#[test]
fn v004_lowercase_flagged_for_is_silent() {
    let t = "The commit was flagged for review by CI.\n";
    let report = run(t, Profile::GeneralWriting);
    assert!(
        !has_rule(&report, "SLOP-V004"),
        "V004 fired on lowercase flagged-for: {:?}",
        common::rule_ids(&report)
    );
}

/// The request-reference entries live on this rule alone, and V004 now
/// applies at candidate in every profile.
#[test]
fn v004_request_reference_is_a_candidate_everywhere() {
    let text = "Renamed the flag as requested in review.\n";
    for profile in Profile::ALL {
        let report = run(text, profile);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-V004")
            .unwrap_or_else(|| panic!("V004 silent in {}", profile.as_str()));
        assert_eq!(f.state, "candidate", "in {}", profile.as_str());
        assert!(
            !report.findings.iter().any(|f| f.rule_id == "SLOP-V003"),
            "V003 must not own the request-reference phrases"
        );
    }
}

/// Segmentation: agent-loop phrases quoted in code formatting are mentions.
#[test]
fn v004_phrases_in_code_formatting_are_silent() {
    let t = "The banned phrase list:\n\n```\nnot rerun in this turn\nFlagged for JJ\n```\n";
    let report = run(t, Profile::GeneralWriting);
    assert!(
        !has_rule(&report, "SLOP-V004"),
        "V004 fired from inside a code fence"
    );
}

// --- SLOP-V005 ledger-stamp --------------------------------------------------

/// The harvested stamp shapes fire as candidates on a durable public
/// surface: the owner-verdict phrase in every form, and each verdict verb
/// directly followed by a bare ISO date.
#[test]
fn v005_ledger_stamps_fire() {
    for text in [
        "The owner rules this on 2026-08-18.\n",
        "The owner ruled this 2026-08-12.\n",
        "Ruled 2026-08-14 after the sweep.\n",
        "The floor was measured 2026-08-01 across the boundary.\n",
        "The digest was verified 2026-07-30.\n",
        "The collision was resolved 2026-07-31.\n",
        "Re-measured 2026-08-02 on the new toolchain.\n",
    ] {
        let report = run(text, Profile::Doc);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-V005")
            .unwrap_or_else(|| panic!("V005 silent on {text:?}"));
        assert_eq!(f.state, "candidate", "{text:?}");
        assert_invariants(text, &report);
    }
}

/// Release-date diction and the prose on-date form stay out of scope, and
/// report relaxes because a dated verification is content there.
#[test]
fn v005_release_diction_and_prose_form_are_silent() {
    for text in [
        "Released 2026-08-18 with two fixes.\n",
        "Published 2026-08-01 on the registry.\n",
        "The floor was measured on 2026-08-01.\n",
        "The audit resolved 2026 budget items.\n",
        "The owner ruled this out after the inspection.\n",
        "If the owner rules this way, we ship Friday.\n",
        "The build was verified 2026-99-99.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            !has_rule(&report, "SLOP-V005"),
            "V005 fired on out-of-scope text {text:?}: {:?}",
            common::rule_ids(&report)
        );
    }
    let stamp = "Ruled 2026-08-14 by the owner.\n";
    let report = run(stamp, Profile::GeneralWriting);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-V005")
        .expect("the stamp is a candidate in general-writing");
    assert_eq!(f.state, "candidate");
    let report = run(stamp, Profile::Report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-V005")
        .expect("still reported under report");
    assert_eq!(f.lifecycle, "advisory", "relax: candidate reports advisory");
}

// --- SLOP-C008 contrastive pair ---------------------------------------------

/// The #414 escape shapes verbatim: the infinitive pair, the wh-parallel
/// pair, the interpolated pair C007 excludes by design, and the
/// two-sentence reframe without C002's pronoun-subject requirement.
#[test]
fn c008_pair_shapes_fire_as_experimental_candidates() {
    for text in [
        "The FRI position is not to dismiss breadth, but to require depth first.\n",
        "The survey asks not what they know about religion, but how they value it.\n",
        "The cache is not a source of truth. It is an optimization.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert_invariants(text, &report);
        let f = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-C008")
            .unwrap_or_else(|| panic!("C008 silent on {text:?}"));
        assert_eq!(f.state, "candidate", "{text:?}");
        assert_eq!(f.lifecycle, "experimental", "{text:?}");
    }
}

/// The directive interpolation is ADJUDICABLE, not silent: it fires
/// candidate and the guard's judge question carries the verdict. Pinned so
/// a future suppression heuristic cannot silently widen.
#[test]
fn c008_directive_interpolation_lands_candidate() {
    let t = "Ship the fix, not the workaround, but tell support first.\n";
    let report = run(t, Profile::GeneralWriting);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C008")
        .expect("interpolated pair must reach the judge");
    assert_eq!(f.state, "candidate");
}

/// Adjacent-family territory stays owned: scheduling prose without the pair
/// shape, adverb-marked not-just forms (C001), and rather-than forms (C003)
/// never fire C008.
#[test]
fn c008_stays_silent_on_adjacent_family_territory() {
    for text in [
        "Submit the form not later than Friday.\n",
        "The tool is not just a linter but a gate.\n",
        "Use exponential backoff rather than fixed sleeps.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(
            !has_rule(&report, "SLOP-C008"),
            "C008 fired on adjacent-family text {text:?}: {:?}",
            common::rule_ids(&report)
        );
    }
}
