//! Regression coverage for the homoglyph fold (the param-coverage gate
//! tests live in policy_ci.rs).
//!
//! Without the fold, `dеlve` with a Cyrillic е (U+0435) exited CLEAN while
//! the ASCII spelling fired SLOP-A001. A reader sees pixel-identical slop
//! and the report is empty. The norm view folds
//! cross-script Latin homoglyphs to Latin INSIDE MIXED-SCRIPT TOKENS ONLY,
//! and SLOP-H003 surfaces the mixed-script token itself as a hint
//! (implementing its previously declared-but-dead unusual_scripts param).

mod common;

use common::{assert_invariants, has_rule, rule_ids, run};
use unslop::Profile;

fn filler() -> String {
    "This paragraph is ordinary visible prose that carries the document. ".repeat(16)
}

// The evasions: homoglyph-hidden lexicon words fire A001 directly ----------

#[test]
fn homoglyph_hidden_lexicon_words_fire() {
    let cases = [
        ("cyrillic е in markdown", "Ordinary d\u{0435}lve detail."),
        (
            "cyrillic е in html",
            "<div>Ordinary d\u{0435}lve detail.</div>",
        ),
        (
            "cyrillic а in game-changer",
            "A real g\u{0430}me-ch\u{0430}nger here.",
        ),
        (
            "multiple homoglyphs in one token",
            "Ordinary d\u{0435}lv\u{0435} detail.",
        ),
        ("greek ο in markdown", "The team will delve int\u{03BF} it."),
    ];
    for (label, src) in cases {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-A001"),
            "{label} must fold and fire A001: {:?}",
            rule_ids(&report)
        );
        assert!(
            has_rule(&report, "SLOP-H003"),
            "{label} must also surface the mixed-script hint: {:?}",
            rule_ids(&report)
        );
        assert_invariants(&text, &report);
    }
}

// Guardrails ----------------------------------------------------------------

#[test]
fn pure_cyrillic_text_is_never_folded() {
    // Genuine Russian: every token is single-script, so nothing folds and
    // neither the lexicon nor the mixed-script hint fires.
    let text = format!(
        "{f}\n\nЭто репозиторий с примерами кода и документацией.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    for rule in ["SLOP-A001", "SLOP-H003"] {
        assert!(
            !has_rule(&report, rule),
            "pure-Cyrillic prose must stay untouched, fired {rule}: {:?}",
            rule_ids(&report)
        );
    }
    assert_invariants(&text, &report);
}

#[test]
fn accented_latin_is_not_a_confusable() {
    // é is Latin script — not a cross-script homoglyph. No fold, no hint.
    let text = format!("{f}\n\nMeet at the café tomorrow morning.\n", f = filler());
    let report = run(&text, Profile::Doc);
    for rule in ["SLOP-A001", "SLOP-H003"] {
        assert!(
            !has_rule(&report, rule),
            "café must stay untouched, fired {rule}: {:?}",
            rule_ids(&report)
        );
    }
}

#[test]
fn mixed_script_non_lexicon_token_hints_but_does_not_fire() {
    // A mixed token that folds to a non-lexicon word: the H003 hint surfaces
    // the oddity, but no violation fires coincidentally.
    let text = format!("{f}\n\nThe GmbН filing arrived yesterday.\n", f = filler());
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-H003"),
        "mixed-script token must hint: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "no coincidental lexicon hit: {:?}",
        rule_ids(&report)
    );
}

// P003 dagger guard promise -------------------------------------------------

#[test]
fn dagger_footnote_definition_line_is_exempt_inline_still_fires() {
    // Inline dagger-digit residue fires.
    let text = format!(
        "{f}\n\nThe api call†2 returns the record set.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-P003"),
        "inline dagger-digit residue must fire: {:?}",
        rule_ids(&report)
    );
    // A dagger-digit pair that begins its line is a footnote definition.
    let text = format!(
        "{f}\n\nThe api call returns the record set.\n\n†2 See the appendix for the full schema.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-P003"),
        "a footnote-defining line must be exempt: {:?}",
        rule_ids(&report)
    );
}

// Fold-then-match: the four classes a mixed-script-only (ASCII-letter
// witness) guard leaves open, each a confirmed silent FN without the
// fully-foldable path.

fn state_of<'r>(report: &'r unslop::Report, id: &str) -> Option<&'r str> {
    report
        .findings
        .iter()
        .find(|f| f.rule_id == id)
        .map(|f| f.state.as_str())
}

#[test]
fn all_homoglyph_words_fire_at_candidate_tier() {
    // Whole words spelled from the fold's own table (no ASCII letter): the
    // fully-foldable path folds them and an exact lexicon hit fires at
    // CANDIDATE tier — conservative for the rare genuine-foreign collision.
    let cases = [
        ("моѕаіс", "The design is a моѕаіс of ideas.", "SLOP-A010"),
        ("ерітоме", "It is the ерітоме of care.", "SLOP-A010"),
        ("νаѕт", "A νаѕт улучшение overall.", "SLOP-I004"),
        ("воаѕтѕ", "The tool воаѕтѕ a parser.", "SLOP-O002"),
    ];
    for (label, src, rule) in cases {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert_eq!(
            state_of(&report, rule),
            Some("candidate"),
            "{label} must fire {rule} at candidate tier: {:?}",
            rule_ids(&report)
        );
        assert_invariants(&text, &report);
    }
}

#[test]
fn nfkc_compatibility_spellings_fire_hard() {
    // Fullwidth Latin and mathematical alphanumerics NFKC-normalize to plain
    // ASCII inside word tokens: the identifier-security fold, hard A001.
    for (label, src) in [
        ("fullwidth", "We ｄｅｌｖｅ into the topic."),
        ("math alphanumerics", "We 𝖽𝖾𝗅𝗏𝖾 into the topic."),
    ] {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert_eq!(
            state_of(&report, "SLOP-A001"),
            Some("violation"),
            "{label} must fire hard A001: {:?}",
            rule_ids(&report)
        );
        assert_invariants(&text, &report);
    }
}

#[test]
fn html_split_homoglyph_fuses_and_fires() {
    // `d<b>е</b>lve` arrives as three HTML text pieces; the fold runs on the
    // FUSED norm text, so it fires exactly like the ASCII baseline
    // `de<b></b>lve` does — no split-token asymmetry.
    let text = format!(
        "{f}\n\n<div>Ordinary d<b>\u{0435}</b>lve detail.</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert_eq!(
        state_of(&report, "SLOP-A001"),
        Some("violation"),
        "HTML-split homoglyph must fuse and fire: {:?}",
        rule_ids(&report)
    );
    assert_invariants(&text, &report);
}

#[test]
fn entity_decoded_homoglyph_folds_too() {
    // `d&#1077;lve` decodes to the Cyrillic е (an ordinary printable) and the
    // post-build fold judges the fused token, closing the layered evasion.
    let text = format!("{f}\n\nOrdinary d&#1077;lve detail.\n", f = filler());
    let report = run(&text, Profile::Doc);
    assert_eq!(
        state_of(&report, "SLOP-A001"),
        Some("violation"),
        "ref-decoded homoglyph must fold: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn fold_then_match_guardrails_hold() {
    // Genuine Russian (contains non-homoglyph letters), accented Latin,
    // a fully-foldable token whose fold is NOT a lexicon word (токен →
    // "token"), and legitimate compatibility chars: none may fire.
    let cases = [
        "Это репозиторий с примерами кода и документацией для разработчиков.",
        "Meet at the café for a naïve test.",
        "The токен parser runs.",
        "Use ½ cup and the ﬁnal offset.",
    ];
    for src in cases {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        for rule in ["SLOP-A001", "SLOP-A002", "SLOP-I004", "SLOP-O002"] {
            assert!(
                !has_rule(&report, rule),
                "guardrail {src:?} must not fire {rule}: {:?}",
                rule_ids(&report)
            );
        }
        assert_invariants(&text, &report);
    }
}
