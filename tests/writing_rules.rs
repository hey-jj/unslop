//! The rules the writing profiles introduced: the whole dash family, the
//! semicolon stance split, the process-narration half of the first-person
//! family, and the config-driven License section.

mod common;

use common::{has_rule, run};
use unslop::{analyze, Config, Deployment, Profile};

fn state_of<'a>(r: &'a unslop::Report, id: &str) -> Option<&'a str> {
    r.findings
        .iter()
        .find(|f| f.rule_id == id)
        .map(|f| f.state.as_str())
}

// --- SLOP-M001: the dash family ---------------------------------------------

#[test]
fn every_dash_substitution_fires() {
    for text in [
        "The room went quiet — nobody moved.\n",
        "The file – which is long – took an hour.\n",
        "The Paris–London route closed.\n",
        "The report--long as it was--landed.\n",
        "The plan - such as it was - held.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(
            has_rule(&report, "SLOP-M001"),
            "M001 missed a dash substitution in {text:?}"
        );
    }
}

#[test]
fn numeric_ranges_and_hyphenated_words_are_silent() {
    for text in [
        "Between 2020–2024 the rate fell.\n",
        "Pages 3 - 5 cover the method.\n",
        "The well-known result holds.\n",
        "She wrote a first-person account.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(
            !has_rule(&report, "SLOP-M001"),
            "M001 fired on legitimate punctuation in {text:?}"
        );
    }
}

#[test]
fn the_dash_suggestion_puts_back_the_letters_it_consumed() {
    let text = "The plan - such as it was - held. A dash — here.\n";
    let mut config = Config::new(Profile::GeneralWriting);
    config.suggest = true;
    let report = analyze(text.as_bytes(), &config).unwrap();
    let fixes: Vec<(String, String)> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-M001")
        .map(|f| {
            let s = f.suggestion.as_ref().expect("mechanical rules suggest");
            (text[s.start..s.end].to_string(), s.replace_with.clone())
        })
        .collect();
    assert_eq!(fixes.len(), 3, "three dash findings: {fixes:?}");
    // The two letter-anchored patterns consume a neighbour, so the fix hands
    // the letters back. The em dash matches alone and needs no repair.
    assert_eq!(fixes[0], ("n - s".to_string(), "n, s".to_string()));
    assert_eq!(fixes[1], ("s - h".to_string(), "s, h".to_string()));
    assert_eq!(fixes[2], ("—".to_string(), ", ".to_string()));
}

// --- SLOP-M002: the semicolon stance split ----------------------------------

#[test]
fn semicolons_block_in_doc_report_to_a_writer_and_are_off_elsewhere() {
    let text = "The plan is simple; the work is not.\n";
    assert_eq!(
        state_of(&run(text, Profile::Doc), "SLOP-M002"),
        Some("violation"),
        "doc blocks, where the reader is following instructions"
    );
    assert_eq!(
        state_of(&run(text, Profile::Report), "SLOP-M002"),
        Some("candidate"),
        "report asks the writer to answer for it"
    );
    // Everywhere else a semicolon is a writer's choice, not a generation
    // signal, so the rule says nothing at all.
    for profile in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
    ] {
        assert_eq!(
            state_of(&run(text, profile), "SLOP-M002"),
            None,
            "in {}",
            profile.as_str()
        );
    }
}

/// The shape the ruling was written against: a draft whose only marks are
/// semicolons ships from a voice profile, reports once per semicolon in a
/// report, and blocks in a doc.
#[test]
fn a_two_semicolon_draft_ships_from_a_voice_profile() {
    let text = "The plan is simple; the work is not. She left early; nobody minded.\n";

    let general = run(text, Profile::GeneralWriting);
    assert_eq!(general.result_state, "no_findings");
    assert_eq!(general.exit_code(), 0);

    let report = run(text, Profile::Report);
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.rule_id == "SLOP-M002" && f.state == "candidate")
            .count(),
        2,
        "one adjudicable candidate per semicolon"
    );

    let doc = run(text, Profile::Doc);
    assert_eq!(doc.result_state, "violations_present");
    assert_eq!(doc.exit_code(), 10);
}

// --- SLOP-F001 and SLOP-F004: the first-person split ------------------------

#[test]
fn process_narration_fires_where_plain_first_person_does_not() {
    let narrated = "I ran the numbers twice before writing this.\n";
    for profile in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
    ] {
        let report = run(narrated, profile);
        assert!(
            has_rule(&report, "SLOP-F004"),
            "F004 silent in {}",
            profile.as_str()
        );
        assert!(
            !has_rule(&report, "SLOP-F001"),
            "F001 must be off in {}",
            profile.as_str()
        );
    }
}

#[test]
fn plain_first_person_is_content_in_the_voice_profiles() {
    let text = "I grew up two streets from the river.\n";
    for profile in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::SocialPost,
    ] {
        let report = run(text, profile);
        assert!(
            !has_rule(&report, "SLOP-F001") && !has_rule(&report, "SLOP-F004"),
            "the first-person family fired on plain voice in {}",
            profile.as_str()
        );
    }
    assert!(has_rule(&run(text, Profile::Doc), "SLOP-F001"));
}

#[test]
fn present_tense_first_person_is_not_process_narration() {
    for text in [
        "I run three miles most mornings.\n",
        "We check the mail on Fridays.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(
            !has_rule(&report, "SLOP-F004"),
            "F004 fired on present tense in {text:?}"
        );
    }
}

#[test]
fn perfect_and_contracted_forms_fire() {
    for text in [
        "We have verified the totals against the ledger.\n",
        "I've checked the totals twice.\n",
        "We had tested the claim before publishing.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert!(has_rule(&report, "SLOP-F004"), "F004 missed {text:?}");
    }
}

// --- SLOP-K005: the License section -----------------------------------------

fn doc_config(expected: Option<&str>) -> Config {
    let mut config = Config::new(Profile::Doc);
    config.deployment = Deployment {
        expected_license_wording: expected.map(|s| s.to_string()),
        ..Deployment::default()
    };
    config
}

#[test]
fn a_document_without_a_license_section_is_silent_until_config_asks() {
    let text = b"# Guide\n\nThe loader reads one file per run.\n";
    let report = analyze(text, &doc_config(None)).unwrap();
    assert!(
        !has_rule(&report, "SLOP-K005"),
        "K005 must not demand a License section on its own"
    );

    let report = analyze(text, &doc_config(Some("MIT or Apache-2.0"))).unwrap();
    assert_eq!(state_of(&report, "SLOP-K005"), Some("violation"));
}

#[test]
fn a_license_section_is_checked_against_the_configured_wording() {
    let text = b"# Guide\n\nThe loader reads one file per run.\n\n## License\n\nMIT or Apache-2.0, at your option.\n";
    let report = analyze(text, &doc_config(Some("MIT or Apache-2.0"))).unwrap();
    assert!(
        !has_rule(&report, "SLOP-K005"),
        "matching wording must pass"
    );

    let report = analyze(text, &doc_config(Some("GPL-3.0 only"))).unwrap();
    assert_eq!(state_of(&report, "SLOP-K005"), Some("violation"));

    // Without configured wording the rule reports what it could not check.
    let report = analyze(text, &doc_config(None)).unwrap();
    assert_eq!(state_of(&report, "SLOP-K005"), Some("coverage_hint"));
}
