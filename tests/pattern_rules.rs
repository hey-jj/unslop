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
        Profile::Essay,
        &[
            "The deal set the stage for a longer partnership.\n",
            "The custom is deeply rooted in the valley.\n",
            "She left a lasting legacy at the school.\n",
        ],
    );
    assert_silent(
        "SLOP-A006",
        Profile::Essay,
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
        Profile::Essay,
        &[
            "The substrate of the argument never changed.\n",
            "Our vantage of the year is narrow.\n",
            "The primitives of the craft are three.\n",
        ],
    );
    // Outside the frame the same words are usually meant literally.
    assert_silent(
        "SLOP-A008",
        Profile::Essay,
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
        Profile::Essay,
        &[
            "We reviewed numerous drafts prior to the meeting.\n",
            "In order to file, commence the process online.\n",
        ],
    );
    assert_silent(
        "SLOP-A009",
        Profile::Essay,
        &["We reviewed many drafts before the meeting.\n"],
    );

    let mut config = Config::new(Profile::Essay);
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
        Profile::Essay,
        &[
            "Despite challenges, the shop continues to thrive.\n",
            "The team weathered the storm and came out stronger.\n",
        ],
    );
    assert_silent(
        "SLOP-O006",
        Profile::Essay,
        &["The shop lost two suppliers in March and replaced one in June.\n"],
    );
}

// --- anchored rules ---------------------------------------------------------

#[test]
fn o005_participial_tail_needs_the_block_final_position() {
    assert_fires(
        "SLOP-O005",
        Profile::Essay,
        &[
            "The team cut load time by half, demonstrating its commitment to speed.\n",
            "The council met twice, ensuring every objection was heard.\n",
        ],
    );
    assert_silent(
        "SLOP-O005",
        Profile::Essay,
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
        Profile::Essay,
        &["If you are coming from automation: instead of handlers, you name conditions.\n"],
    );
    assert_silent(
        "SLOP-M007",
        Profile::Essay,
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
        Profile::Essay,
        &[
            "The book covers everything from philosophy to cooking.\n",
            "Her work ranges from portraiture to civic planning.\n",
        ],
    );
    assert_silent(
        "SLOP-C010",
        Profile::Essay,
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
        Profile::Essay,
        &["The work was featured in Wired, The Atlantic, and Vogue.\n"],
    );
    assert_silent(
        "SLOP-O007",
        Profile::Essay,
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
        Profile::Essay,
        &["Progress was slow this year.\n\nThe future looks bright.\n"],
    );
    assert_silent(
        "SLOP-O008",
        Profile::Essay,
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
        Profile::Essay,
        &[
            "The file is parsed by the loader.\n",
            "The totals have been checked by the auditor.\n",
            "The decision was made by the committee.\n",
        ],
    );
    assert_silent(
        "SLOP-L001",
        Profile::Essay,
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
    let report = run(text, Profile::Essay);
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
    assert!(fires(&long, Profile::Essay, "SLOP-L003"), "45-word floor");
    let commas = "The plan, which was late, which was over budget, which nobody read, failed.\n";
    assert!(fires(commas, Profile::Essay, "SLOP-L003"), "clause commas");
    assert_silent(
        "SLOP-L003",
        Profile::Essay,
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
        Profile::Essay,
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
        Profile::Essay,
        "SLOP-S001"
    ));
    assert_silent(
        "SLOP-S001",
        Profile::Essay,
        &["The summary generated by the survey team arrived late.\n"],
    );
}

// --- report surface ---------------------------------------------------------

#[test]
fn every_finding_carries_a_container_label() {
    let text = "# The Report On Progress\n\n> A quoted line with delve inside.\n\nWe delve here.\n";
    let report = run(text, Profile::Essay);
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
    let report = run(clean, Profile::Essay);
    assert_eq!(report.result_state, "no_findings");
    assert!(report.coverage.density.word_count > 0);
    assert_eq!(report.coverage.density.byte_len, clean.len());
    assert!(report.coverage.density.families.is_empty());

    let slop = "We delve into the vibrant tapestry. We delve again.\n";
    let report = run(slop, Profile::Essay);
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
        Profile::Essay,
        &["Work stalled.\n\nThe future looks brighter after the repair.\n"],
    );
    assert_fires(
        "SLOP-O008",
        Profile::Essay,
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
        Profile::Essay,
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
        Profile::Essay,
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
        Profile::Essay,
        &[
            "The gap is eight seats: four in support and four on the night shift.\n",
            "What I learned is this: you pack the extra blanket.\n",
            "The rule covers the following: you file before the deadline.\n",
        ],
    );
    assert_fires(
        "SLOP-M007",
        Profile::Essay,
        &["If you are coming from automation: instead of handlers, you name conditions.\n"],
    );
}

/// Calendar words are not outlets, and the list can sit on either side of the
/// attribution.
#[test]
fn o007_reads_the_inverted_listing_and_skips_the_calendar() {
    assert_fires(
        "SLOP-O007",
        Profile::Essay,
        &["Wired, The Atlantic, and Vogue covered the work last spring.\n"],
    );
    assert_silent(
        "SLOP-O007",
        Profile::Essay,
        &["The story was reported by Reuters and Bloomberg on Tuesday morning.\n"],
    );
}

/// The false-range rule needs a trigger and yields to both suppressions.
#[test]
fn c010_arms_and_suppressions() {
    assert_fires(
        "SLOP-C010",
        Profile::Essay,
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
        Profile::Essay,
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
    let blocked = run("The tapestry of voices was seamless.\n", Profile::Essay);
    assert!(blocked
        .findings
        .iter()
        .any(|f| f.rule_id == "SLOP-A001" && f.state == "violation"));
    let reported = run("Unlock is a file on a YubiKey.\n", Profile::Essay);
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
