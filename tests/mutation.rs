//! Section 12.2: mutation tests. Enumerated surface variants (entity,
//! escape, zero-width insertion, CRLF, soft-break split) must still be
//! caught in the norm view.

mod common;

use common::{has_rule, run};
use unslop::Profile;

#[test]
fn em_dash_surface_variants_are_caught() {
    for text in [
        "A dash — here.",
        "A dash &mdash; here.",
        "A dash &#8212; here.",
        "A dash &#x2014; here.",
        "A spaced -- double here.",
        "word--word here.",
    ] {
        let report = run(text, Profile::Doc);
        assert!(has_rule(&report, "SLOP-M001"), "missed: {text}");
    }
}

#[test]
fn semicolon_variants_are_caught() {
    for text in [
        "One; two.",
        "One&#59; two.",
        "Fullwidth； two.",
        "Greek question mark\u{037E} two.",
    ] {
        let report = run(text, Profile::Doc);
        assert!(has_rule(&report, "SLOP-M002"), "missed: {text}");
    }
}

#[test]
fn zero_width_insertion_does_not_evade_the_lexicon() {
    for text in [
        "We de\u{200B}lve into it.",
        "We de\u{200C}lve into it.",
        "We del\u{2060}ve into it.",
    ] {
        let report = run(text, Profile::Doc);
        assert!(has_rule(&report, "SLOP-A001"), "missed: {text:?}");
        // The zero-width character in a word is itself a violation.
        assert!(has_rule(&report, "SLOP-M004"), "M004 missed: {text:?}");
    }
}

#[test]
fn crlf_and_soft_break_variants_are_caught() {
    let crlf = "First line.\r\nWe delve here — with a dash.\r\n";
    let report = run(crlf, Profile::Doc);
    assert!(has_rule(&report, "SLOP-A001"));
    assert!(has_rule(&report, "SLOP-M001"));

    // A phrase split by a soft line break folds to one space in norm.
    let split = "This is a game\nchanger for sure.\n";
    let report = run(split, Profile::Doc);
    assert!(has_rule(&report, "SLOP-A001"), "soft-break split missed");
}

#[test]
fn markdown_escape_resolution_reaches_patterns() {
    // \* escapes resolve in norm; an escaped semicolon entity still lands.
    let text = "Escaped \\*star\\* and one&semi; two.\n";
    let report = run(text, Profile::Doc);
    assert!(has_rule(&report, "SLOP-M002"));
}

#[test]
fn opening_however_fires_only_at_block_start() {
    let opening = "However, this fires.\n";
    let report = run(opening, Profile::Doc);
    assert!(has_rule(&report, "SLOP-M003"));

    let sentence_start = "It works. However, it fires here too.\n";
    let report = run(sentence_start, Profile::Doc);
    assert!(has_rule(&report, "SLOP-M003"));

    let mid = "It works well however you spin it.\n";
    let report = run(mid, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M003"),
        "mid-sentence however fired"
    );
}

#[test]
fn typographic_quotes_fire_only_inside_code() {
    let in_code = "Run `let s = “x”;` to see it.\n";
    let report = run(in_code, Profile::Doc);
    assert!(has_rule(&report, "SLOP-P005"));

    let in_prose = "He said “hello” politely.\n";
    let report = run(in_prose, Profile::Doc);
    assert!(!has_rule(&report, "SLOP-P005"), "prose quotes fired P005");
}

#[test]
fn quotation_downgrade_turns_style_violations_into_candidates() {
    let text = "> compiler said: expected `;`, found — this\n";
    let report = run(text, Profile::Doc);
    let m001 = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-M001")
        .expect("dash detected in quote");
    assert_eq!(m001.state, "candidate");
    assert_eq!(m001.provenance, "claimed-quotation");
}
