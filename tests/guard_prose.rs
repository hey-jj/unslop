//! The guard-prose gate. Rule guards and judge questions are hand-written
//! prose that ships to a reader inside `references/rules.md`, and nothing was
//! checking them. This test reads every `guard` and `judge` string out of the
//! loaded package and fails the run on the mechanical writing classes.
//!
//! The scope is by field on purpose. Patterns, match tables, words, and
//! lexicons are never read, so a regex literal or a lexicon entry cannot raise
//! a finding and no exemption has to be written to excuse one. The gate simply
//! never sees them.
//!
//! One exemption applies to the phrase classes. A guard legitimately names the
//! terms its rule matches, so a hit whose text is a term declared anywhere in
//! the package is a mention and passes. The exemption never reaches the
//! punctuation classes, because no rule declares a semicolon or a dash, so
//! those have no path out. A mention is exempt and a use is rewritten, and
//! neither is a way to make a finding disappear.

use std::collections::BTreeSet;
use unslop::policy;

/// The contrast-scaffolding shapes the gate reads, as a closed list. The
/// widening path is one class per round behind a measured pass, with
/// contrastive negation the next one up.
const SCAFFOLDING: &[&str] = &[
    "on one hand",
    "on the one hand",
    "on the other hand",
    "best of both worlds",
    "without sacrificing",
    "no silver bullet",
    "one-size-fits-all",
    "strikes a balance",
    "strike a balance",
    "all while",
    "the good news is",
    "the bad news is",
    "no compromise",
];

/// Every literal term the package declares: resolved lexicon entries, inline
/// words, exemption collocations, and every string sitting in a rule's params.
/// This is the mention set, and it is built from the package rather than from
/// the files so it can never fall out of step with what the rules actually
/// load.
fn declared_terms(pkg: &policy::PolicyPackage) -> BTreeSet<String> {
    fn walk(v: &toml::Value, out: &mut BTreeSet<String>) {
        match v {
            toml::Value::String(s) => {
                out.insert(s.to_lowercase());
            }
            toml::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            toml::Value::Table(t) => t.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    for rule in &pkg.rules {
        for t in &rule.terms {
            out.insert(t.to_lowercase());
        }
        for e in &rule.exemptions {
            out.insert(e.to_lowercase());
        }
        walk(&rule.params, &mut out);
    }
    out
}

/// True when `at` starts and `at + needle.len()` ends on a word boundary.
fn word_bounded(hay: &str, at: usize, len: usize) -> bool {
    let before = hay[..at].chars().next_back();
    let after = hay[at + len..].chars().next();
    !before.is_some_and(|c| c.is_alphanumeric()) && !after.is_some_and(|c| c.is_alphanumeric())
}

/// Scan one guard or judge string. Returns one message per finding, each
/// naming the rule and the substring that has to change. `filler` is the
/// repository's own banned-filler set, read from the rule that owns it.
fn scan(
    rule_id: &str,
    field: &str,
    text: &str,
    filler: &[String],
    declared: &BTreeSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    // Punctuation. No exemption reaches this half.
    for (class, mark) in [
        ("em dash", '\u{2014}'),
        ("en dash", '\u{2013}'),
        ("semicolon", ';'),
    ] {
        if text.contains(mark) {
            out.push(format!(
                "{rule_id} {field}: {class} in guard prose ({mark:?})"
            ));
        }
    }
    // Phrase classes, each exempt where the package declares the phrase.
    let lower = text.to_lowercase();
    let phrases = filler
        .iter()
        .map(|s| ("banned filler", s.as_str()))
        .chain(SCAFFOLDING.iter().map(|s| ("contrast scaffolding", *s)));
    for (class, phrase) in phrases {
        if declared.contains(phrase) {
            continue; // a guard naming a term its package declares is a mention
        }
        let mut at = 0usize;
        while let Some(pos) = lower[at..].find(phrase) {
            let s = at + pos;
            if word_bounded(&lower, s, phrase.len()) {
                out.push(format!("{rule_id} {field}: {class} {phrase:?}"));
            }
            at = s + 1;
        }
    }
    out
}

fn filler_terms(pkg: &policy::PolicyPackage) -> Vec<String> {
    pkg.rule_by_id("SLOP-T001")
        .expect("the filler rule owns the banned-filler set")
        .terms
        .iter()
        .map(|t| t.to_lowercase())
        .collect()
}

#[test]
fn every_guard_and_judge_string_passes_the_mechanical_classes() {
    let pkg = policy::load().unwrap();
    let declared = declared_terms(&pkg);
    let filler = filler_terms(&pkg);
    let mut found = Vec::new();
    for rule in &pkg.rules {
        found.extend(scan(&rule.id, "guard", &rule.guard, &filler, &declared));
        if let Some(judge) = &rule.judge {
            found.extend(scan(&rule.id, "judge", judge, &filler, &declared));
        }
    }
    assert!(
        found.is_empty(),
        "guard prose carries {} mechanical violation(s):\n  {}",
        found.len(),
        found.join("\n  ")
    );
}

/// The negative control. A synthetic guard carrying one instance of each class
/// goes through the same function and has to come back with all of them, so a
/// green run above means the gate looked rather than that it cannot see.
#[test]
fn the_gate_catches_a_synthetic_guard() {
    let pkg = policy::load().unwrap();
    let declared = declared_terms(&pkg);
    let filler = filler_terms(&pkg);

    let bad = "The rule fires here \u{2014} and there \u{2013} on one hand; \
               it strikes a balance without sacrificing speed.";
    let found = scan("SLOP-TEST", "guard", bad, &filler, &declared);
    for want in [
        "em dash",
        "en dash",
        "semicolon",
        "\"on one hand\"",
        "\"strikes a balance\"",
        "\"without sacrificing\"",
    ] {
        assert!(
            found.iter().any(|f| f.contains(want)),
            "the gate missed {want} in the control: {found:?}"
        );
    }
    assert!(found.iter().all(|f| f.starts_with("SLOP-TEST guard: ")));

    // A clean string produces nothing, so the control is not simply always on.
    assert!(scan(
        "SLOP-TEST",
        "guard",
        "The rule reads the clause and reports the span.",
        &filler,
        &declared
    )
    .is_empty());
}

/// The mention exemption is real and is scoped away from punctuation. A guard
/// naming a term its own rule declares passes, the same words undeclared do
/// not, and a semicolon has no exemption path at all.
#[test]
fn a_mention_is_exempt_and_punctuation_never_is() {
    let pkg = policy::load().unwrap();
    let declared = declared_terms(&pkg);
    let filler = filler_terms(&pkg);

    // SLOP-T001's guard names three of its own entries and stays green.
    let t001 = pkg.rule_by_id("SLOP-T001").unwrap();
    assert!(t001.guard.to_lowercase().contains("at its core"));
    assert!(scan("SLOP-T001", "guard", &t001.guard, &filler, &declared).is_empty());

    // The same phrase with nothing declaring it is a finding.
    let undeclared = ["on one hand"];
    assert!(!declared.contains(undeclared[0]));
    assert!(!scan(
        "SLOP-X",
        "guard",
        "It works on one hand.",
        &filler,
        &declared
    )
    .is_empty());

    // No rule declares a semicolon, so the punctuation class cannot be
    // exempted even when the rest of the sentence is a mention.
    assert!(!declared.contains(";"));
    let semi = scan(
        "SLOP-X",
        "guard",
        "It reads at its core; it reports.",
        &filler,
        &declared,
    );
    assert_eq!(semi.len(), 1);
    assert!(semi[0].contains("semicolon"));
}
