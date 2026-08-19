//! Section 12.11: for each homograph the listed collocations pass, the bare
//! prose use blocks, and a testing crate's readme using "test harness"
//! passes end to end. delve and game-changer block unconditionally.
//! `harness` is narrowed to the verb-with-object slop form —
//! every noun use passes structurally, no exemption list needed.

mod common;

use common::{has_rule, run};
use unslop::Profile;

fn a002_fires(text: &str) -> bool {
    run(text, Profile::Doc)
        .findings
        .iter()
        .any(|f| f.rule_id == "SLOP-A002" && f.state == "violation")
}

#[test]
fn exempt_collocations_pass() {
    for text in [
        "The test harness runs nightly.",
        "A fuzz harness covers the parser.",
        "The orchestration harness deploys containers.",
        "The CI harness caches builds.",
        "The harness ran to completion.",
        "Set the authentication realm before login.",
        "The kerberos realm name is EXAMPLE.ORG.",
        "Navigate to the settings page.",
        "Print in landscape orientation.",
        "Choose portrait or landscape.",
    ] {
        assert!(!a002_fires(text), "exempt collocation fired: {text}");
    }
}

#[test]
fn bare_prose_uses_block() {
    for text in [
        "You can harness the power of the type system.",
        "It harnesses your existing configuration.",
        "Harnessing its potential requires no setup.",
        "The realm of possibilities is wide.",
        "Users navigate complexity with ease.",
        "The testing landscape keeps changing.",
    ] {
        assert!(a002_fires(text), "bare use did not fire: {text}");
    }
}

#[test]
fn delve_and_game_changer_block_unconditionally() {
    for text in [
        "We delve into the internals.",
        "This is a game-changer for parsing.",
        "A game changer, in every test harness.",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-A001"),
            "SLOP-A001 did not fire: {text}"
        );
    }
}

#[test]
fn test_harness_readme_passes_end_to_end() {
    let text = "# `check-rig`\n\nA test harness for property checks. The test harness \
                config lives in `rig.toml`.\n\n## License\n\nMIT or Apache-2.0.\n";
    let report = run(text, Profile::Doc);
    let violations: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.state == "violation")
        .map(|f| f.rule_id.as_str())
        .collect();
    assert!(violations.is_empty(), "violations: {violations:?}");
}

#[test]
fn highly_available_is_exempt_but_bare_highly_fires() {
    let ok = run("The cluster is highly available.", Profile::Doc);
    assert!(!has_rule(&ok, "SLOP-I001"));
    let bad = run("The cluster is highly efficient.", Profile::Doc);
    assert!(has_rule(&bad, "SLOP-I001"));
}
