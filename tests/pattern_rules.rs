//! The pattern rules the writing profiles need: puffery, promotional
//! diction, metaphor nouns, plain-word swaps, participial tails, colons as
//! connectors, formulaic challenge arcs, false ranges, name-dropping,
//! generic conclusions, agentive passive, dense sentences, title-case
//! headings, boldface density, curly quotes, and the courtesy splits. Each
//! rule gets the shape it fires on and the shape it must stay silent on.

mod common;

use common::{has_rule, run};
use unslop::{analyze, Config, Profile};

fn fires(text: &str, profile: Profile, id: &str) -> bool {
    has_rule(&run(text, profile), id)
}

fn assert_fires(id: &str, profile: Profile, cases: &[&str]) {
    for text in cases {
        assert!(fires(text, profile, id), "{id} missed {text:?}");
    }
}

fn assert_silent(id: &str, profile: Profile, cases: &[&str]) {
    for text in cases {
        assert!(!fires(text, profile, id), "{id} fired on {text:?}");
    }
}

// --- lexicon rules ----------------------------------------------------------

#[test]
fn a006_puffery() {
    assert_fires(
        "SLOP-A006",
        Profile::GeneralWriting,
        &[
            "The deal set the stage for a longer partnership.\n",
            "The custom is deeply rooted in the valley.\n",
            "She left a lasting legacy at the school.\n",
        ],
    );
    assert_silent(
        "SLOP-A006",
        Profile::GeneralWriting,
        &[
            "The deal cut the filing deadline from 30 days to 10.\n",
            "The custom started in 1890 and the valley kept it.\n",
        ],
    );
}

#[test]
fn a007_promotional_language() {
    assert_fires(
        "SLOP-A007",
        Profile::SocialPost,
        &[
            "The nestled village is a hidden gem.\n",
            "A breathtaking, must-visit stop on the coast.\n",
        ],
    );
    assert_silent(
        "SLOP-A007",
        Profile::SocialPost,
        &["The village sits two miles from the cliffs and has one bus stop.\n"],
    );
}

#[test]
fn a008_metaphor_nouns_need_the_of_frame() {
    assert_fires(
        "SLOP-A008",
        Profile::GeneralWriting,
        &[
            "The substrate of the argument never changed.\n",
            "Our vantage of the year is narrow.\n",
            "The primitives of the craft are three.\n",
        ],
    );
    // Outside the frame the same words are usually meant literally.
    assert_silent(
        "SLOP-A008",
        Profile::GeneralWriting,
        &[
            "The policy is the substrate for every later decision.\n",
            "The vector pointed north.\n",
            "Scaffolding went up on Tuesday.\n",
        ],
    );
    // Off in doc, relaxed in report: a doc that says vector usually means one.
    let text = "The substrate of the argument never changed.\n";
    assert!(!fires(text, Profile::Doc, "SLOP-A008"));
    let report = run(text, Profile::Report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A008")
        .expect("report still reports it");
    assert_eq!(f.lifecycle, "advisory", "relax: candidate reports advisory");
}

#[test]
fn a009_plain_word_swaps_carry_their_fix() {
    assert_fires(
        "SLOP-A009",
        Profile::GeneralWriting,
        &[
            "We reviewed numerous drafts prior to the meeting.\n",
            "In order to file, commence the process online.\n",
        ],
    );
    assert_silent(
        "SLOP-A009",
        Profile::GeneralWriting,
        &["We reviewed many drafts before the meeting.\n"],
    );

    let mut config = Config::new(Profile::GeneralWriting);
    config.suggest = true;
    let report = analyze(b"We reviewed Numerous drafts.\n", &config).unwrap();
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A009")
        .expect("swap fires");
    let fix = f.suggestion.as_ref().expect("swap carries its replacement");
    assert_eq!(fix.replace_with, "Many", "the fix matches the matched case");
}

#[test]
fn o006_formulaic_challenges() {
    assert_fires(
        "SLOP-O006",
        Profile::GeneralWriting,
        &[
            "Despite challenges, the shop continues to thrive.\n",
            "The team weathered the storm and came out stronger.\n",
        ],
    );
    assert_silent(
        "SLOP-O006",
        Profile::GeneralWriting,
        &["The shop lost two suppliers in March and replaced one in June.\n"],
    );
}

// --- anchored rules ---------------------------------------------------------

#[test]
fn o005_participial_tail_needs_the_block_final_position() {
    assert_fires(
        "SLOP-O005",
        Profile::GeneralWriting,
        &[
            "The team cut load time by half, demonstrating its commitment to speed.\n",
            "The council met twice, ensuring every objection was heard.\n",
        ],
    );
    assert_silent(
        "SLOP-O005",
        Profile::GeneralWriting,
        &[
            // Mid-block: the clause does not close the block.
            "The council met twice, ensuring every objection was heard. Two passed.\n",
            // Opening a sentence rather than closing one.
            "Ensuring every objection was heard, the council met twice.\n",
            // No comma, so no tail.
            "The council met twice ensuring every objection was heard.\n",
        ],
    );
}

#[test]
fn m007_colon_as_connector() {
    assert_fires(
        "SLOP-M007",
        Profile::GeneralWriting,
        &["If you are coming from automation: instead of handlers, you name conditions.\n"],
    );
    assert_silent(
        "SLOP-M007",
        Profile::GeneralWriting,
        &[
            // A colon introducing a list has no word after it on the line.
            "The rule covers three things:\n\n- one\n- two\n- three\n",
            // A label reads as a label.
            "Note: the log rotates hourly.\n",
            // Not enough before the colon to be a clause.
            "Wait: this matters.\n",
            // A bold lead-in is the inline-header shape another rule owns.
            "- **Performance**: the parser got faster this year for everyone.\n",
        ],
    );
}

#[test]
fn c010_false_range_needs_a_range_signal_and_unscaled_endpoints() {
    assert_fires(
        "SLOP-C010",
        Profile::GeneralWriting,
        &[
            "The book covers everything from philosophy to cooking.\n",
            "Her work ranges from portraiture to civic planning.\n",
        ],
    );
    assert_silent(
        "SLOP-C010",
        Profile::GeneralWriting,
        &[
            // Real scales.
            "Attendance ranges from 30 to 200 people.\n",
            "The season spans from March to September.\n",
            // No range signal: movement, not a claimed scale.
            "The walk took us from London to Dover in two days.\n",
        ],
    );
}

#[test]
fn o007_name_dropping_is_anchored_three_ways() {
    assert_fires(
        "SLOP-O007",
        Profile::GeneralWriting,
        &["The work was featured in Wired, The Atlantic, and Vogue.\n"],
    );
    assert_silent(
        "SLOP-O007",
        Profile::GeneralWriting,
        &[
            // No attribution trigger.
            "The team met Ana, Ben, and Chris at the depot.\n",
            // Fewer than three names.
            "The work was featured in Wired and Vogue.\n",
            // A sentence that quotes one source is doing the work.
            "The work was covered by Wired, The Atlantic, and Vogue, which called it \"a mess\".\n",
        ],
    );
}

#[test]
fn o008_generic_conclusion_is_anchored_to_the_last_block() {
    assert_fires(
        "SLOP-O008",
        Profile::GeneralWriting,
        &["Progress was slow this year.\n\nThe future looks bright.\n"],
    );
    assert_silent(
        "SLOP-O008",
        Profile::GeneralWriting,
        &[
            // Mid-document the same line is a passing remark.
            "The future looks bright.\n\nThe budget closes on Friday.\n",
            "The team ships CSV export in September and audit logs in October.\n",
        ],
    );
}

// --- plain speech -----------------------------------------------------------

#[test]
fn l001_agentive_passive_only_with_the_actor_present() {
    assert_fires(
        "SLOP-L001",
        Profile::GeneralWriting,
        &[
            "The file is parsed by the loader.\n",
            "The totals have been checked by the auditor.\n",
            "The decision was made by the committee.\n",
        ],
    );
    assert_silent(
        "SLOP-L001",
        Profile::GeneralWriting,
        &[
            // No actor: nothing to move to the front.
            "The file is parsed at startup.\n",
            // Adjectival, which is why bare is-plus-participle stays out.
            "The door is closed and the results are mixed.\n",
            "The loader parses the file.\n",
        ],
    );
}

#[test]
fn l002_passive_density_reports_a_rate_and_never_gates() {
    let text = "The file is parsed by the loader. The totals were checked by the auditor.\n";
    let report = run(text, Profile::GeneralWriting);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-L002")
        .expect("the rate instrument reports");
    assert_eq!(f.state, "coverage_hint");
    assert_eq!(f.lifecycle, "advisory");
    assert!(f.message.contains("per 1000 words"), "{}", f.message);
    assert_ne!(report.result_state, "violations_present");
}

#[test]
fn l003_dense_sentence_counts_words_and_clause_commas() {
    let long = format!(
        "The {} ended.\n",
        "committee reviewed the second consultation and ".repeat(9)
    );
    assert!(
        fires(&long, Profile::GeneralWriting, "SLOP-L003"),
        "45-word floor"
    );
    let commas = "The plan, which was late, which was over budget, which nobody read, failed.\n";
    assert!(
        fires(commas, Profile::GeneralWriting, "SLOP-L003"),
        "clause commas"
    );
    assert_silent(
        "SLOP-L003",
        Profile::GeneralWriting,
        &[
            "The plan failed.\n",
            // Commas inside numbers are not clause commas.
            "The budget was 1,024,000 and the total was 2,048,000.\n",
        ],
    );
}

// --- presentation -----------------------------------------------------------

#[test]
fn e004_title_case_heading() {
    assert_fires(
        "SLOP-E004",
        Profile::BlogPost,
        &["# Building The Parser From Source\n\nText.\n"],
    );
    assert_silent(
        "SLOP-E004",
        Profile::BlogPost,
        &[
            "# Building the parser from source\n\nText.\n",
            // The first word is capitalized either way.
            "# The parser\n\nText.\n",
        ],
    );
}

#[test]
fn e005_boldface_density_stays_out_of_the_bold_label_rule() {
    assert_fires(
        "SLOP-E005",
        Profile::BlogPost,
        &["The **parser** reads the **file** and writes a **report** for the team.\n"],
    );
    assert_silent(
        "SLOP-E005",
        Profile::BlogPost,
        &[
            "The **parser** reads the file and writes a **report**.\n",
            // Leading bold labels across list items belong to SLOP-E003.
            "- **Fast**: quick\n- **Safe**: sound\n- **Clean**: neat\n",
        ],
    );
}

#[test]
fn m008_curly_quotes_fire_in_doc_only() {
    let text = "The report is \u{201C}ready\u{201D} for review.\n";
    assert!(fires(text, Profile::Doc, "SLOP-M008"), "silent in doc");
    for p in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::Report,
        Profile::SocialPost,
    ] {
        assert!(!fires(text, p, "SLOP-M008"), "fired in {}", p.as_str());
    }
}

// --- the courtesy splits ----------------------------------------------------

#[test]
fn email_keeps_the_assistant_half_and_drops_the_human_half() {
    let assistant = "I hope this helps. Let me know if you'd like more.\n";
    let human = "Best regards, and thanks for your time. Feel free to reach out.\n";
    for (id, text) in [
        ("SLOP-S003", assistant),
        ("SLOP-V003", assistant),
        ("SLOP-S004", human),
        ("SLOP-S005", human),
        ("SLOP-V006", human),
    ] {
        assert!(fires(text, Profile::Doc, id), "{id} silent in doc");
    }
    assert!(fires(assistant, Profile::Email, "SLOP-S003"));
    assert!(fires(assistant, Profile::Email, "SLOP-V003"));
    for id in ["SLOP-S004", "SLOP-S005", "SLOP-V006"] {
        assert!(!fires(human, Profile::Email, id), "{id} fired in email");
    }
}

#[test]
fn s001_attribution_is_anchored_to_block_start() {
    assert!(fires(
        "Generated by an assistant.\n",
        Profile::GeneralWriting,
        "SLOP-S001"
    ));
    assert_silent(
        "SLOP-S001",
        Profile::GeneralWriting,
        &["The summary generated by the survey team arrived late.\n"],
    );
}

// --- report surface ---------------------------------------------------------

#[test]
fn every_finding_carries_a_container_label() {
    let text = "# The Report On Progress\n\n> A quoted line with delve inside.\n\nWe delve here.\n";
    let report = run(text, Profile::GeneralWriting);
    let labeled: Vec<(&str, &str)> = report
        .findings
        .iter()
        .map(|f| (f.rule_id.as_str(), f.container.as_str()))
        .collect();
    assert!(labeled.contains(&("SLOP-E004", "heading")), "{labeled:?}");
    assert!(
        labeled.contains(&("SLOP-A001", "blockquote")),
        "{labeled:?}"
    );
    assert!(labeled.contains(&("SLOP-A001", "prose")), "{labeled:?}");
    for (id, container) in &labeled {
        assert!(
            ["prose", "heading", "blockquote", "fenced-code"].contains(container),
            "{id} carries an unknown container {container}"
        );
    }
}

#[test]
fn the_density_block_describes_the_document_and_never_gates() {
    let clean = "The rain arrived late on Thursday and stayed for two days.\n";
    let report = run(clean, Profile::GeneralWriting);
    assert_eq!(report.result_state, "no_findings");
    assert!(report.coverage.density.word_count > 0);
    assert_eq!(report.coverage.density.byte_len, clean.len());
    assert!(report.coverage.density.families.is_empty());

    let slop = "We delve into the vibrant tapestry. We delve again.\n";
    let report = run(slop, Profile::GeneralWriting);
    let ornamental = report
        .coverage
        .density
        .families
        .iter()
        .find(|f| f.family == "ornamental")
        .expect("the family appears once it has findings");
    assert!(ornamental.findings >= 3);
    assert!(
        ornamental.per_1000_words.contains('.'),
        "rate carries one decimal: {}",
        ornamental.per_1000_words
    );
}

// --- review-round defects, each pinned to the sentence that found it --------

/// The lexicon matches on word boundaries, so a comparative is a different
/// sentence. "The future looks brighter" reported as "The future looks bright".
#[test]
fn o008_matches_whole_words_only() {
    assert_silent(
        "SLOP-O008",
        Profile::GeneralWriting,
        &["Work stalled.\n\nThe future looks brighter after the repair.\n"],
    );
    assert_fires(
        "SLOP-O008",
        Profile::GeneralWriting,
        &["Work stalled.\n\nThe future looks bright.\n"],
    );
}

/// A trailing sign-off does not move where the writing ends.
#[test]
fn o008_looks_past_a_trailing_signoff() {
    assert_fires(
        "SLOP-O008",
        Profile::Email,
        &[
            "The audit closed.\n\nThe future looks bright.\n\nBest regards,\nSam\n",
            "The audit closed.\n\nThe future looks bright.\n\nSam\n",
        ],
    );
}

/// Irregular participles and actors outside ASCII both count.
#[test]
fn l001_reads_irregular_participles_and_any_actor() {
    assert_fires(
        "SLOP-L001",
        Profile::GeneralWriting,
        &[
            "The answer is known by everyone.\n",
            "The song was sung by the choir.\n",
            "The letter was written by the clerk.\n",
            "The invoice was checked by \u{00C9}lodie.\n",
        ],
    );
}

/// A by-phrase naming a time is a deadline, not an actor.
#[test]
fn l001_ignores_a_temporal_by_phrase() {
    assert_silent(
        "SLOP-L001",
        Profile::GeneralWriting,
        &[
            "The work was completed by Friday.\n",
            "The report was finished by noon.\n",
            "The move was done by then.\n",
            "The form was signed by 5pm.\n",
        ],
    );
}

/// A soft line wrap is not a block start. The word after one is mid-sentence,
/// and no position-anchored rule may fire there.
#[test]
fn a_soft_wrap_does_not_start_a_block() {
    assert_silent(
        "SLOP-S004",
        Profile::Doc,
        &["She signed the letter\nsincerely after the meeting.\n"],
    );
    // A real sign-off still fires from its own line.
    assert_fires(
        "SLOP-S004",
        Profile::Doc,
        &["Thanks for reading.\n\nSincerely,\nSam\n"],
    );
    assert_silent(
        "SLOP-T002",
        Profile::Doc,
        &["The council met on Tuesday and the vote\nadditionally covered the levy.\n"],
    );
}

/// A colon before a two-item join is a list, and the cataphoric colon points
/// forward at an example.
#[test]
fn m007_reads_lists_and_the_cataphoric_form() {
    assert_silent(
        "SLOP-M007",
        Profile::GeneralWriting,
        &[
            "The gap is eight seats: four in support and four on the night shift.\n",
            "What I learned is this: you pack the extra blanket.\n",
            "The rule covers the following: you file before the deadline.\n",
        ],
    );
    assert_fires(
        "SLOP-M007",
        Profile::GeneralWriting,
        &["If you are coming from automation: instead of handlers, you name conditions.\n"],
    );
}

/// Calendar words are not outlets, and the list can sit on either side of the
/// attribution.
#[test]
fn o007_reads_the_inverted_listing_and_skips_the_calendar() {
    assert_fires(
        "SLOP-O007",
        Profile::GeneralWriting,
        &["Wired, The Atlantic, and Vogue covered the work last spring.\n"],
    );
    assert_silent(
        "SLOP-O007",
        Profile::GeneralWriting,
        &["The story was reported by Reuters and Bloomberg on Tuesday morning.\n"],
    );
}

/// The false-range rule needs a trigger and yields to both suppressions.
#[test]
fn c010_arms_and_suppressions() {
    assert_fires(
        "SLOP-C010",
        Profile::GeneralWriting,
        &[
            // Arm A, a breadth signal anywhere in the sentence.
            "The book covers everything from philosophy to cooking.\n",
            "Her work ranges from portraiture to civic planning.\n",
            // Arm B, a category head immediately before from.
            "Topics from algebra to poetry appear in one week.\n",
        ],
    );
    assert_silent(
        "SLOP-C010",
        Profile::GeneralWriting,
        &[
            // Suppression 1, a quantity endpoint.
            "Attendance ranges from 30 to 200 people.\n",
            "The season spans from March to September.\n",
            // Suppression 2, a motion or conversion verb before from.
            "The team moved from London to Dover last spring.\n",
            "We converted from JSON to YAML in one pass.\n",
            // No trigger at all.
            "The syllabus moves from Homer to hip-hop.\n",
            // The through form is a connector everywhere.
            "The course runs from Monday through Friday.\n",
        ],
    );
}

/// The ornamental split: a word with no plain sense left blocks, a word that
/// keeps one reports.
#[test]
fn the_ornamental_split_tiers_by_whether_a_plain_sense_survives() {
    let blocked = run(
        "The tapestry of voices was seamless.\n",
        Profile::GeneralWriting,
    );
    assert!(blocked
        .findings
        .iter()
        .any(|f| f.rule_id == "SLOP-A001" && f.state == "violation"));
    let reported = run("Unlock is a file on a YubiKey.\n", Profile::GeneralWriting);
    assert!(reported
        .findings
        .iter()
        .any(|f| f.rule_id == "SLOP-A010" && f.state == "candidate"));
    assert_ne!(reported.result_state, "violations_present");
}

/// Every word the vision names as AI vocabulary produces a finding somewhere.
#[test]
fn the_named_vocabulary_is_covered() {
    for (word, sentence) in [
        ("additionally", "Additionally, the team met.\n"),
        ("crucial", "This is a crucial step.\n"),
        ("delve", "The book delves into the topic.\n"),
        ("enduring", "It has enduring appeal.\n"),
        ("enhance", "The change will enhance the page.\n"),
        ("fostering", "The scheme is fostering growth.\n"),
        ("garner", "The plan will garner support.\n"),
        ("interplay", "The interplay of the two is clear.\n"),
        ("intricate", "The intricate design held.\n"),
        ("landscape", "The landscape has shifted.\n"),
        ("pivotal", "It was a pivotal week.\n"),
        ("showcase", "The gallery showcases the results.\n"),
        ("tapestry", "A tapestry of voices.\n"),
        ("testament", "It is a testament to her work.\n"),
        ("underscore", "These numbers underscore the point.\n"),
        ("vibrant", "The vibrant square filled.\n"),
    ] {
        let report = run(sentence, Profile::Doc);
        assert!(
            !report.findings.is_empty(),
            "{word} produced no finding at all"
        );
    }
}

// --- input boundary, sentence boundary, and verb tense ---------------------

/// Source code is not prose. Gating a source file draws findings from
/// statement punctuation, so the boundary fails closed at exit 40.
#[test]
fn source_code_is_rejected_at_the_input_boundary() {
    let src = concat!(
        "//! A module that is not prose.\n",
        "use std::io;\n",
        "\n",
        "#[derive(Debug)]\n",
        "pub struct Loader {\n",
        "    path: String,\n",
        "}\n",
        "\n",
        "impl Loader {\n",
        "    pub fn new(path: String) -> Loader {\n",
        "        Loader { path }\n",
        "    }\n",
        "}\n",
    );
    for profile in Profile::ALL {
        let err = analyze(src.as_bytes(), &Config::new(profile)).unwrap_err();
        let unslop::AnalysisError::UnsupportedInput(m) = &err else {
            panic!("{} accepted source: {err:?}", profile.as_str());
        };
        // The message names the shape it read, the real counts, and both
        // remedies, so a reader knows what to do without reading the guard.
        assert!(m.starts_with("Input looks like a Rust source file:"), "{m}");
        assert!(m.contains("10 of 11 lines"), "{m}");
        assert!(
            m.contains("Pass the prose, or wrap the code in a fenced block."),
            "{m}"
        );
    }
}

/// The ruled thresholds: eight lines of code structure, and 35 percent of the
/// non-blank lines outside code blocks. A document under either bar is prose.
#[test]
fn the_guard_holds_both_thresholds() {
    // Seven code lines is under the line floor however dense the file is.
    let seven = concat!(
        "use std::io;\n",
        "use std::fmt;\n",
        "use std::mem;\n",
        "use std::ops;\n",
        "use std::cmp;\n",
        "use std::env;\n",
        "use std::net;\n",
    );
    assert!(
        analyze(seven.as_bytes(), &Config::new(Profile::Doc)).is_ok(),
        "seven code lines tripped the line floor"
    );

    // Nine code lines carried by twenty lines of prose is under the ratio.
    let mut mixed = String::new();
    for i in 0..20 {
        mixed.push_str(&format!(
            "The loader reads one file per run, and this is sentence {i}.\n\n"
        ));
    }
    for m in ["io", "fmt", "mem", "ops", "cmp", "env", "net", "str", "vec"] {
        mixed.push_str(&format!("use std::{m};\n\n"));
    }
    assert!(
        analyze(mixed.as_bytes(), &Config::new(Profile::Doc)).is_ok(),
        "prose carrying nine code lines tripped the ratio"
    );
}

/// The field-line shape is tight on purpose. A definition list writes several
/// words after its colon and is prose, so it must not read as a field.
#[test]
fn a_definition_list_is_not_a_field_line() {
    let mut doc = String::from("# Terms\n\nEach term below names one thing.\n\n");
    for (term, gloss) in [
        ("name", "the person who signed the record"),
        ("reason", "the sentence a reader would accept"),
        ("span", "the bytes the finding covers"),
        ("state", "what the finding does to the exit code"),
        ("profile", "the writing the draft is meant to be"),
        ("container", "where in the document the span sits"),
        ("digest", "the hash over the whole policy package"),
        ("expiry", "the date after which the waiver lapses"),
        ("signer", "the person who takes responsibility"),
        ("remedy", "the edit that makes the finding untrue"),
    ] {
        doc.push_str(&format!("{term}: {gloss},\n"));
    }
    assert!(
        analyze(doc.as_bytes(), &Config::new(Profile::Doc)).is_ok(),
        "a definition list read as source"
    );

    // The same lines behind a bullet marker are prose too.
    let bulleted = doc.replace("\nname:", "\n- name:");
    assert!(
        analyze(bulleted.as_bytes(), &Config::new(Profile::Doc)).is_ok(),
        "a bulleted definition list read as source"
    );
}

/// A document that quotes code is a document. The prose and code split is the
/// extractor's own, so every fence style and an indented block all count as
/// code rather than as prose lines.
#[test]
fn a_document_that_quotes_code_stays_prose() {
    let backtick = concat!(
        "# Guide\n\nRun the loader like this.\n\n",
        "```rust\n",
        "use std::io;\n#[derive(Debug)]\npub struct Loader {\n    path: String,\n}\n",
        "impl Loader {\n    pub fn new() -> Loader {\n        Loader {}\n    }\n}\n",
        "```\n\nThe loader reads one file per run.\n",
    );
    let tilde = backtick.replace("```rust", "~~~rust").replace("```", "~~~");
    let indented = concat!(
        "# Guide\n\nRun the loader like this.\n\n",
        "    use std::io;\n    #[derive(Debug)]\n    pub struct Loader {\n",
        "        path: String,\n    }\n    impl Loader {\n",
        "        pub fn new() -> Loader {\n            Loader {}\n        }\n    }\n",
        "\nThe loader reads one file per run.\n",
    );
    for (label, text) in [
        ("backtick fence", backtick.to_string()),
        ("tilde fence", tilde),
        ("indented block", indented.to_string()),
    ] {
        assert!(
            analyze(text.as_bytes(), &Config::new(Profile::Doc)).is_ok(),
            "{label} was rejected as source"
        );
    }
}

/// The staged-agreement arm consumes a sentence boundary, and the boundary
/// has to be a real one in both directions: a list marker is not a sentence
/// end, and a sentence that ends on a numeral is.
#[test]
fn c004_needs_a_real_sentence_boundary() {
    assert_silent(
        "SLOP-C004",
        Profile::GeneralWriting,
        &["See the docs, e.g. granted, the flag is set, but the cache stays cold.\n"],
    );
    assert_fires(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            "It shipped early. Granted, the code is shorter, but it hides the cost.\n",
            // A numeral can end a sentence. This is the direction the blanket
            // digit test had backwards.
            "The stable protocol is version 2. Granted, the code is shorter, but it hides the cost.\n",
            "The list has 2. Granted, the code is shorter, but it hides the cost.\n",
        ],
    );
}

/// All eight praise entries are anchored to the opening of a sentence, a
/// line, or a list item, because that is where performed agreement lives.
/// The set is anchored whole, so no entry is left as a way around the
/// discrimination. Decoration in front of the phrase does not move it.
#[test]
fn v002_anchors_the_eight_praise_entries() {
    assert_fires(
        "SLOP-V002",
        Profile::Comment,
        &[
            "Great question. The parser reads the file.\n",
            "Good question. The parser reads the file.\n",
            "Excellent question. The parser reads the file.\n",
            "That's a great question. The parser reads the file.\n",
            "Great point. The parser reads the file.\n",
            "- Excellent point. The parser reads the file.\n",
            "You're absolutely right. The parser reads the file.\n",
            "You are absolutely right. The parser reads the file.\n",
            // A party popper is decoration, and the block-start test reads
            // past it, so the phrase stays where the reader sees it.
            "\u{1F389} Great question! The parser reads the file.\n",
        ],
    );
    assert_silent(
        "SLOP-V002",
        Profile::Comment,
        &[
            "he asked a great question\n",
            "He made a great point about the allocator.\n",
            "She raised an excellent question about the allocator.\n",
            "The reviewer called it an excellent point and moved on.\n",
            "I said you are absolutely right to worry about it.\n",
        ],
    );
}

/// The emoji before the phrase is decoration and the block-start test looks
/// past it, so an anchored entry keeps its position and the emoji reports on
/// its own rule. SLOP-T001's block-start entry was broken the same way and
/// recovers with the same fix.
#[test]
fn leading_decoration_does_not_move_the_block_start() {
    let text = "\u{1F389} Great question! The parser reads the file.\n";
    let report = run(text, Profile::Comment);
    assert!(has_rule(&report, "SLOP-V002"), "the praise entry moved");
    assert!(has_rule(&report, "SLOP-M006"), "the emoji reports itself");

    for text in [
        "\u{1F389} Overall, the design works.\n",
        "\u{2705} Overall, the design works.\n",
        "Overall, the design works.\n",
    ] {
        assert!(
            fires(text, Profile::Doc, "SLOP-T001"),
            "T001 lost its block-start entry behind decoration: {text:?}"
        );
    }
    // A comma or a dash is not decoration: a word behind one is mid-sentence
    // and the test must still say so.
    assert_silent(
        "SLOP-T001",
        Profile::Doc,
        &["The build is green, overall the design works.\n"],
    );
}

/// A bullet carried in from a rendered list is a marker, so the block-start
/// test reads past it and an anchored phrase keeps its position. The four
/// glyphs are listed one by one because the ranges around them split by
/// accident: the geometric shapes a nested level uses were already
/// transparent and the plain bullet was not.
#[test]
fn a_pasted_bullet_does_not_move_the_block_start() {
    for glyph in ['\u{2022}', '\u{2023}', '\u{2043}', '\u{2219}', '\u{25E6}'] {
        let text = format!("{glyph} Great question! The parser reads the file.\n");
        assert!(
            fires(&text, Profile::Comment, "SLOP-V002"),
            "a praise opener moved behind {glyph:?}"
        );
        let overall = format!("{glyph} Overall, the design works.\n");
        assert!(
            fires(&overall, Profile::Doc, "SLOP-T001"),
            "T001 lost its opener behind {glyph:?}"
        );
    }
    // U+00B7 is a letter in Catalan and a separator in running prose, not
    // what a rendered list pastes as, so it gets no transparency.
    assert_silent(
        "SLOP-T001",
        Profile::Doc,
        &["\u{00B7} Overall, the design works.\n"],
    );
}

/// Text mode has no parser to strip list markers, so the extractor opens the
/// prose range past the marker run itself. Without that, every rule anchored
/// to a block start read the position after the marker rather than the
/// position a reader sees.
#[test]
fn text_mode_reads_past_a_list_marker() {
    let fires_text = |text: &str, id: &str| {
        let mut cfg = Config::new(Profile::Doc);
        cfg.input_format = unslop::InputFormat::Text;
        analyze(text.as_bytes(), &cfg)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.rule_id == id)
    };
    for (marker, id, text) in [
        ("dash", "SLOP-T001", "- Overall, the design works.\n"),
        ("star", "SLOP-M003", "* However, the parser is faster.\n"),
        ("plus", "SLOP-T002", "+ Moreover, the parser is faster.\n"),
        (
            "bullet",
            "SLOP-T001",
            "\u{2022} Overall, the design works.\n",
        ),
        (
            "ordered paren",
            "SLOP-T001",
            "1) Overall, the design works.\n",
        ),
        ("quote", "SLOP-S001", "> Generated by an AI assistant.\n"),
        ("heading", "SLOP-S004", "# Best regards,\n"),
    ] {
        assert!(
            fires_text(text, id),
            "{marker}-led line: {id} stayed silent"
        );
    }
    // A marker glyph with no space after it is ordinary text, so a rule, a
    // negative number, and a hashtag keep their bytes.
    let mut cfg = Config::new(Profile::Doc);
    cfg.input_format = unslop::InputFormat::Text;
    for text in ["---\n", "-5 degrees below Overall\n", "#tag Overall here\n"] {
        let report = analyze(text.as_bytes(), &cfg).unwrap();
        assert!(
            !report.findings.iter().any(|f| f.rule_id == "SLOP-T001"),
            "a non-marker was stripped: {text:?}"
        );
    }
}

/// good catch and great catch left the lexicon. Their honest use opens a
/// line, so anchoring would have discriminated nothing. The paired residue
/// still reports, on the anchored phrase beside them.
#[test]
fn v002_no_longer_reads_a_reviewer_catching_something() {
    assert_silent(
        "SLOP-V002",
        Profile::Comment,
        &[
            "Good catch.\n",
            "Great catch.\n",
            "Good catch, the offset was off by one.\n",
        ],
    );
    let text = "Good catch! You're absolutely right.\n";
    let report = run(text, Profile::Comment);
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-V002")
        .map(|f| serde_json::from_str::<String>(f.snippet.get()).unwrap())
        .collect();
    assert_eq!(spans, vec!["You're absolutely right"]);

    let words = include_str!("../policy/words/assistant-voice.txt").to_ascii_lowercase();
    for phrase in ["good catch", "great catch", "nice catch", "makes sense"] {
        assert!(
            !words.lines().any(|l| l.trim() == phrase),
            "{phrase:?} must stay out of the assistant register"
        );
    }
}

/// fair hit is the concession entry and carries no anchor, because position
/// tells nothing apart here: opening a reply with it and writing it mid
/// sentence are the same tell. Two collisions ride on it and both reach the
/// judge rather than an exemption.
#[test]
fn v002_reads_fair_hit_wherever_it_sits() {
    assert_fires(
        "SLOP-V002",
        Profile::Comment,
        &[
            "Fair hit. I'll revise the section.\n",
            "That's a fair hit on the design.\n",
            "Fair hit on the naming, I'll change it.\n",
            // Both collisions fire and are answered at the judge question:
            // the literal sense, and the word that contains the entry.
            "The replay showed a fair hit to the shoulder.\n",
            "That was an unfair hit.\n",
        ],
    );
    // The neighbours the ruling keeps out: three set phrases and a cricket
    // idiom, none of which is the concession this entry reads.
    assert_silent(
        "SLOP-V002",
        Profile::Comment,
        &[
            "Fair point, I'll change it.\n",
            "Fair enough.\n",
            "It's a fair cop.\n",
            "That was a fair knock.\n",
        ],
    );
    let words = include_str!("../policy/words/assistant-voice.txt").to_ascii_lowercase();
    assert!(words.lines().any(|l| l.trim() == "fair hit"));
    for phrase in ["fair callout", "fair ding", "fair point", "fair knock"] {
        assert!(
            !words.lines().any(|l| l.trim() == phrase),
            "{phrase:?} is not in this round"
        );
    }
}

/// The line-start arm splits by word. although and though have no temporal
/// reading and fire unqualified. A while clause drops on either of the two
/// shapes that mark time passing, a progressive before the comma or a
/// participle straight after the keyword. The bare durative present is the
/// stated miss and still fires.
#[test]
fn c004_tells_concessive_while_from_temporal_while() {
    assert_silent(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            // The progressive drop.
            "While you are working, you might notice unexpected changes.\n",
            "While the loader is reading the manifest, the cache stays cold.\n",
            "While we were waiting, the build finished.\n",
            // The participial drop: a temporal while-participle takes an
            // activity verb.
            "While working on the migration, we found a race.\n",
            "While redistributing the build, we hit a limit.\n",
            "While reviewing the diff, the reader loses the thread.\n",
        ],
    );
    assert_fires(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            "While the parser is slower, it handles more cases.\n",
            // The stated miss: a durative present with no progressive.
            "While the build runs, grab a coffee.\n",
            // although and though never take either test.
            "Although you are working, the changes land anyway.\n",
            "Though we were waiting, the build finished.\n",
        ],
    );
    // One adverb may stand between the be-form and the -ing word, which is
    // where a writer puts still or already. Two may not.
    assert_silent(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            "While we were already running the testsuite, the build stayed green.\n",
            "While the loader is still reading the manifest, the cache stays cold.\n",
            "While the job is currently running, the queue holds.\n",
            "While the parser is quietly building the tree, the reader waits.\n",
        ],
    );
    assert_fires(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            // Two tokens in the gap: the pair is no longer a verb group.
            "While we were already quietly running the tests, the build stayed green.\n",
            // An adverb in the gap but no participle after it.
            "While the API is now stable, the docs lag behind.\n",
        ],
    );

    // The eight concession participles are exempt from the participial drop,
    // because a concessive while-participle takes a verb of cognition.
    for verb in [
        "acknowledging",
        "recognizing",
        "granting",
        "accepting",
        "conceding",
        "admitting",
        "noting",
        "allowing",
    ] {
        let text = format!("While {verb} the risk, the team proceeded anyway.\n");
        assert!(
            fires(&text, Profile::GeneralWriting, "SLOP-C004"),
            "the concession exemption lost {verb}"
        );
    }
}

/// The participial drop asks for a participle heading the clause, and a finite
/// verb inside the clause says the -ing word is doing something else there.
/// The set is closed and carries no morphology, because an -s scan reads every
/// plural noun as a verb and an -ed scan reads every participial adjective as
/// one.
#[test]
fn c004_participial_drop_needs_a_clause_with_no_finite_verb() {
    // A finite verb means the -ing word modifies a noun or stands as a
    // subject. Both of these were measured as concessions the drop lost
    // before the gate existed.
    assert_fires(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            "While programming language parsers are usually written manually, nom differs.\n",
            "While manipulating ASTs is the most flexible way, iterators are easier.\n",
            // One entry from each row of the closed set.
            "While it can be slow, it is correct.\n",
            "While the parsing rules have changed, the output has not.\n",
            "While running totals do drift, the ledger reconciles.\n",
        ],
    );
    // be, been, and being are deliberately out of the finite set, so a
    // participial clause built on them still drops.
    assert_silent(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            "While being tested, the parser reports every span.\n",
            "While working on the migration, the team found a race.\n",
        ],
    );
    // The clause is cut at its own first comma, not at the last comma the
    // greedy pattern reached, so a later clause cannot lend it a finite verb.
    assert_silent(
        "SLOP-C004",
        Profile::GeneralWriting,
        &["While redistributing the Work thereof, You may choose to offer, and charge a fee for, acceptance of support.\n"],
    );
}

/// One deny-list of words that end in -ing without being participles, declared
/// once and read by both rules that ask the question. SLOP-C004's while drop
/// is the second reader, so `While nothing changes` keeps firing.
#[test]
fn the_non_participle_ing_list_serves_both_rules() {
    assert_fires(
        "SLOP-C004",
        Profile::GeneralWriting,
        &[
            "While nothing changes, the report still fires.\n",
            "While everything compiles, the tests still fail.\n",
        ],
    );
    // The same list keeps SLOP-C007's trailing tag off the same words.
    assert_fires(
        "SLOP-C007",
        Profile::Doc,
        &["It flags the shape, not everything.\n"],
    );
    assert_silent(
        "SLOP-C007",
        Profile::Doc,
        &["She listened, never judging anyone.\n"],
    );
}

/// A block boundary is a sentence boundary, and a stronger one, so the
/// staged-agreement arm reads across it. What the boundary licenses is the
/// match and never the span: the finding opens at the concession word.
#[test]
fn c004_reads_across_a_block_edge_and_opens_at_the_concession() {
    let two_items =
        "- Land the patch.\n- Granted, the parser is slower, but it handles nested spans.\n";
    let report = run(two_items, Profile::GeneralWriting);
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C004")
        .map(|f| serde_json::from_str::<String>(f.snippet.get()).unwrap())
        .collect();
    assert_eq!(spans.len(), 1, "one concession, one finding: {spans:?}");
    assert!(
        spans[0].starts_with("Granted"),
        "span opened before the concession word: {:?}",
        spans[0]
    );
    assert!(!spans[0].contains('\n'), "span crossed a block edge");

    // The same figure inside one block reports the same way, with the
    // licensing period left out of the span.
    let one_block = "It shipped early. Granted, the code is shorter, but it hides the cost.\n";
    let report = run(one_block, Profile::GeneralWriting);
    let span = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C004")
        .map(|f| serde_json::from_str::<String>(f.snippet.get()).unwrap())
        .expect("C004 silent inside one block");
    assert!(span.starts_with("Granted"), "span was {span:?}");
}

/// A concession opening a list item reports, and the two input formats now
/// agree on it. Text mode strips the marker the way the markdown parser
/// always did, so the same bytes give the same finding and the same span
/// either way. Before the marker strip, text mode read `2. ` as prose and the
/// digit-list-marker suppression kept the arm off it, which is the divergence
/// this pins closed.
#[test]
fn c004_reads_list_markers_and_block_edges_correctly() {
    let marker = "2. Granted, the code is shorter, but it hides the cost.\n";
    let mut text_cfg = Config::new(Profile::GeneralWriting);
    text_cfg.input_format = unslop::InputFormat::Text;
    let as_text = analyze(marker.as_bytes(), &text_cfg).unwrap();
    let as_markdown = run(marker, Profile::GeneralWriting);
    let spans = |r: &unslop::Report| -> Vec<(usize, usize)> {
        r.findings
            .iter()
            .filter(|f| f.rule_id == "SLOP-C004")
            .map(|f| (f.spans[0].start, f.spans[0].end))
            .collect()
    };
    assert_eq!(spans(&as_text), vec![(3, 36)], "text mode");
    assert_eq!(
        spans(&as_text),
        spans(&as_markdown),
        "the two input formats must read a marker-led concession the same way"
    );

    // Across two list items the arm reads the boundary and reports, and no
    // finding drags the previous item into its span.
    let two_items =
        "Steps.\n\n1. Ship it.\n2. Granted, the code is shorter, but it hides the cost.\n";
    let report = run(two_items, Profile::GeneralWriting);
    assert!(
        report.findings.iter().any(|f| f.rule_id == "SLOP-C004"),
        "the concession opening the second item went unreported"
    );
    for f in report.findings.iter().filter(|f| f.rule_id == "SLOP-C004") {
        let snippet: String = serde_json::from_str(f.snippet.get()).unwrap();
        assert!(
            !snippet.contains('\n'),
            "a C004 span crossed a block edge: {snippet:?}"
        );
    }
}

/// SLOP-A002 reads every homograph's past tense the same way it reads the
/// present: harness structurally, navigate and landscape from the word set.
#[test]
fn a002_reads_the_past_tense() {
    assert_fires(
        "SLOP-A002",
        Profile::GeneralWriting,
        &[
            "The team harnessed the momentum.\n",
            "They harnessed it well.\n",
            "She navigated the room.\n",
            "She navigated the strait at dawn.\n",
            "The report landscaped the field of available tools.\n",
        ],
    );
    assert_silent(
        "SLOP-A002",
        Profile::GeneralWriting,
        &[
            "A harnessed horse waited outside.\n",
            "The test harness ran overnight.\n",
            "She navigated to the page.\n",
        ],
    );
}

/// A past participle standing as an adjective is not the verb the rule
/// reads. Three left-context shapes mark the adjective, and the same three
/// hold for every homograph in the set.
#[test]
fn a002_leaves_participial_adjectives_alone() {
    assert_silent(
        "SLOP-A002",
        Profile::GeneralWriting,
        &[
            // Hyphen-joined compound modifier.
            "The ship crossed well-navigated waters.\n",
            // Determiner directly before the participle.
            "The navigated route followed the river.\n",
            "A landscaped garden surrounds the house.\n",
            "A harnessed horse waited outside.\n",
            // An -ly adverb before the participle.
            "The carefully navigated channel appears on the chart.\n",
        ],
    );
}

/// Every collocation exemption carries its past-tense twin, so the honest
/// technical past is exempt exactly as the present is.
#[test]
fn a002_exemptions_cover_both_tenses() {
    for (present, past) in [
        (
            "She navigates to the page.\n",
            "She navigated to the page.\n",
        ),
        (
            "She navigates the file with the arrow keys.\n",
            "She navigated the file with the arrow keys.\n",
        ),
        (
            "She navigates the directory by hand.\n",
            "She navigated the directory by hand.\n",
        ),
        (
            "She navigates the tree from the root.\n",
            "She navigated the tree from the root.\n",
        ),
        (
            "She navigates the DOM to find the node.\n",
            "She navigated the DOM to find the node.\n",
        ),
    ] {
        assert_silent("SLOP-A002", Profile::GeneralWriting, &[present, past]);
    }
}
