//! SLOP-A004 inflated diction, plus the W001 scrub-list change that unbans
//! the bare word AI. The A004 regression anchor is a tool description built
//! from inflated noun stacks, which the rule must flag, against a set of
//! plain technical sentences the rule must leave alone.

mod common;

use common::{assert_invariants, has_rule, run};
use unslop::Profile;

// --- SLOP-A004 inflated diction ---------------------------------------------

/// A tool description carrying both tells: the tool-noun stack "coverage
/// instrument" and the participial noun stack "generated-text defects".
const INFLATED_DESCRIPTION: &str = "Deterministic detector and coverage instrument \
    for generated-text defects in the writing that goes out\n";

#[test]
fn an_inflated_tool_description_fires_inflated_diction() {
    let report = run(INFLATED_DESCRIPTION, Profile::Doc);
    assert_invariants(INFLATED_DESCRIPTION, &report);
    let a004: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A004")
        .collect();
    assert!(
        a004.len() >= 2,
        "both noun stacks must fire: {:?}",
        common::rule_ids(&report)
    );
    assert!(a004.iter().all(|f| f.state == "candidate"));
    let spans: Vec<&str> = a004
        .iter()
        .map(|f| &INFLATED_DESCRIPTION[f.spans[0].start..f.spans[0].end])
        .collect();
    assert!(spans.contains(&"coverage instrument"), "{spans:?}");
    assert!(spans.contains(&"generated-text defects"), "{spans:?}");
}

#[test]
fn inflated_word_set_fires_on_the_curated_tells() {
    for text in [
        "The service utilizes a queue to schedule work.",
        "This module facilitates communication between the two processes.",
        "The aforementioned flag controls both paths.",
        "We operationalize the checklist in CI.",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(has_rule(&report, "SLOP-A004"), "missed: {text}");
    }
}

/// Plain technical prose, including dense-but-legitimate sentences, must not
/// fire. These are the calibration anchors for list membership.
#[test]
fn plain_technical_prose_does_not_fire_inflated_diction() {
    for text in [
        // A plain report sentence.
        "The survey returns an error when the answer ends mid-sentence.",
        // A plain description of a tool.
        "unslop reads essays, posts, and email before they go out.",
        // Dense but legitimate technical prose.
        "The reverse DFA recovers the start offset for each matched pattern span.",
        // A plain account of a fix.
        "Restarting the worker clears the stale cache entry and the retry succeeds.",
        // Exempt resource metrics, including multi-word collocations.
        "CPU utilization stays under 80 percent under sustained load.",
        "Cache utilization improves when the working set fits in L2.",
        "Connection pool utilization peaked at 92 percent.",
        // The verb sense of instrument.
        "We instrument the allocator to count peak usage.",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(!has_rule(&report, "SLOP-A004"), "false positive on: {text}");
    }
}

/// Homograph senses of the pattern words stay silent: instrument as a
/// measured, financial, or musical noun follows none of the tool-stack
/// modifiers.
#[test]
fn instrument_homographs_do_not_fire() {
    for text in [
        "An oscilloscope is a measurement instrument.",
        "A bond is a financial instrument, not a loan.",
        "The cockpit instrument panel failed during the test.",
        "The cello is a bowed string instrument.",
        "The instrumentation error path exits with code 30.",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(!has_rule(&report, "SLOP-A004"), "false positive on: {text}");
    }
}
