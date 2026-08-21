//! SLOP-C011 proleptic capability denial and SLOP-F005 rationale leak, plus
//! the and-not spelling that joins SLOP-C007. The two specimens are the
//! sentences that reached a shipped README before either rule existed, and
//! they are pinned here verbatim.

mod common;

use common::{assert_invariants, has_rule, run, snippet};
use unslop::Profile;

/// The README line that carried the denial stack.
const SPECIMEN_ONE: &str =
    "It reads text. It does not detect authorship, and no finding is evidence \
     that a person or a model wrote anything.\n";

/// The README paragraph that carried the design argument.
const SPECIMEN_TWO: &str = "Input that is a Rust source file is rejected as unsupported, exit 40, because gating source draws findings from statement punctuation and not from writing. The test reads Rust shape only. Source in another language reaches the rules and produces findings a reader should discount, which is the trade for a guard that never fires on prose. Either pass the prose, or wrap the code in a fenced block, which segmentation excludes.\n";

#[test]
fn specimen_one_fires_the_denial_rule() {
    let report = run(SPECIMEN_ONE, Profile::Doc);
    assert_invariants(SPECIMEN_ONE, &report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C011")
        .expect("C011 silent on the denial specimen");
    assert_eq!(f.state, "candidate");
    assert_eq!(f.family, "contrast");
    // One finding per qualifying clause, and the restatement is in neither.
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .map(snippet)
        .collect();
    assert_eq!(
        spans,
        vec![
            "It does not detect authorship",
            "no finding is evidence that a person or a model wrote anything",
        ]
    );
}

#[test]
fn specimen_two_fires_the_leak_and_the_and_not_spelling() {
    let report = run(SPECIMEN_TWO, Profile::Doc);
    assert_invariants(SPECIMEN_TWO, &report);
    let leaks: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-F005")
        .map(snippet)
        .collect();
    assert_eq!(
        leaks,
        vec!["a reader should discount", "which is the trade"],
        "both marker families report, in document order"
    );
    let c007: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C007")
        .map(snippet)
        .collect();
    assert_eq!(c007, vec!["and not from"]);
}

/// Both rules apply in every profile.
#[test]
fn both_rules_apply_in_every_profile() {
    for profile in Profile::ALL {
        let denial = run(SPECIMEN_ONE, profile);
        assert!(
            has_rule(&denial, "SLOP-C011"),
            "C011 silent in {}",
            profile.as_str()
        );
        let leak = run(SPECIMEN_TWO, profile);
        assert!(
            has_rule(&leak, "SLOP-F005"),
            "F005 silent in {}",
            profile.as_str()
        );
    }
}

/// Arm A: two qualifying clauses in one block need no restatement.
#[test]
fn two_qualifying_clauses_fire_on_their_own() {
    let text = "The linter does not rank writers. The report is not evidence of anything.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(has_rule(&report, "SLOP-C011"));
}

/// The restatement detector reports nothing by itself.
#[test]
fn the_restatement_alone_is_never_a_finding() {
    for text in [
        "It reads text.\n",
        "This processes input.\n",
        "The linter reads the prose.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "C011 fired on a bare restatement: {text:?}"
        );
    }
}

/// P3: the determiner is optional and the two-word verb units match whole.
#[test]
fn restatement_shapes_complete_the_single_clause_trigger() {
    for text in [
        "It reads the text. It does not detect authorship.\n",
        "It looks at input. It does not detect authorship.\n",
        "The tool works with your files. It does not detect authorship.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(has_rule(&report, "SLOP-C011"), "Arm B missed: {text:?}");
    }
}

/// P8 widened the partner from the degenerate restatement to any affirmative
/// clause with a closed-set subject, so a self-description that runs past the
/// object now completes the trigger where the narrow shape would not have.
#[test]
fn any_affirmative_self_description_completes_the_trigger() {
    let text = "It reads text every morning. It does not detect authorship.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(has_rule(&report, "SLOP-C011"));

    // A partner needs a closed-set subject. Without one there is nothing for
    // the denial to sit beside.
    let no_subject = "Rain fell all morning. It does not detect authorship.\n";
    let report = run(no_subject, Profile::Doc);
    assert_invariants(no_subject, &report);
    assert!(!has_rule(&report, "SLOP-C011"));
}

/// P9: the cut falls at every interior coordinator, so a coordinator followed
/// by a subject splits as readily as one followed by a negation.
#[test]
fn every_interior_coordinator_cuts_a_segment() {
    let text = "It does not detect authorship and it never scores voice.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .map(snippet)
        .collect();
    assert_eq!(
        spans,
        vec!["It does not detect authorship", "it never scores voice"]
    );
}

/// P9: each finding names the arm that fired, and an Arm A finding carries
/// the number of denials sharing the block.
#[test]
fn every_finding_names_its_arm() {
    let arm_a = "It does not detect authorship, never scores voice, and makes no claim \
                 about intent.\n";
    let report = run(arm_a, Profile::Doc);
    assert_invariants(arm_a, &report);
    for f in report.findings.iter().filter(|f| f.rule_id == "SLOP-C011") {
        assert_eq!(
            f.message, "proleptic capability denial: arm A, 3 denials in this block",
            "every finding in the stack carries the count"
        );
    }

    let arm_b = "It reads text. It does not detect authorship.\n";
    let report = run(arm_b, Profile::Doc);
    assert_invariants(arm_b, &report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C011")
        .expect("arm B fires");
    assert_eq!(
        f.message,
        "proleptic capability denial: arm B, one denial beside an affirmative partner"
    );
}

/// P9 span rule: a denied capability reports its segment, and an evidential
/// hedge reports the whole comma-delimited clause, because the phrase it
/// matched can run across an interior coordinator.
#[test]
fn the_two_families_report_different_spans() {
    let report = run(SPECIMEN_ONE, Profile::Doc);
    let spans: Vec<(usize, usize)> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .map(|f| (f.spans[0].start, f.spans[0].end))
        .collect();
    assert_eq!(
        spans,
        vec![(15, 44), (50, 112)],
        "family 1 reports its segment, family 2 its clause, uncut at the interior or"
    );
}

/// P8 search order. The qualifying clause looks in its own sentence first,
/// with no distance limit, then at the sentence before and the one after.
#[test]
fn the_partner_search_reads_its_own_sentence_first() {
    // Comma-joined: partner and denial share a sentence.
    let joined = "It reads text, and it does not detect authorship.\n";
    let report = run(joined, Profile::Doc);
    assert_invariants(joined, &report);
    assert!(has_rule(&report, "SLOP-C011"), "own-sentence partner");

    // An interposed clause does not break it, because the within-sentence
    // search has no distance limit.
    let interposed = "It reads text, which is all it was built for, and it does not \
                      detect authorship.\n";
    let report = run(interposed, Profile::Doc);
    assert_invariants(interposed, &report);
    assert!(has_rule(&report, "SLOP-C011"), "distance does not matter");

    // The sentence after works as well as the one before.
    let after = "It does not detect authorship. It reads text.\n";
    let report = run(after, Profile::Doc);
    assert_invariants(after, &report);
    assert!(has_rule(&report, "SLOP-C011"), "following partner");

    // A second denial is no partner, so a lone denial two sentences from its
    // only affirmative stays silent.
    let far = "It reads text. The kettle boiled at noon. It does not detect authorship.\n";
    let report = run(far, Profile::Doc);
    assert_invariants(far, &report);
    assert!(!has_rule(&report, "SLOP-C011"));
}

/// The imperative test runs before either clause family.
#[test]
fn verb_initial_imperatives_stay_silent() {
    for text in [
        "Never author, approve, edit, or sign a waiver.\n",
        "Never cite a finding as evidence that a person wrote something. It reads text.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "C011 fired on an imperative: {text:?}"
        );
    }
}

/// P7: every qualifying clause reports. The three-clause stack yields three
/// findings, one per spelling family that matched.
#[test]
fn the_three_clause_stack_yields_three_findings() {
    let stack = "It does not detect authorship, never scores voice, and makes no claim \
                 about intent.\n";
    let report = run(stack, Profile::Doc);
    assert_invariants(stack, &report);
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .map(snippet)
        .collect();
    assert_eq!(
        spans,
        vec![
            "It does not detect authorship",
            "never scores voice",
            "makes no claim about intent",
        ],
        "spelling A, spelling C, and family 2 each report"
    );
}

/// P7 and P9: a fragment joined without a comma is a segment of its own, so
/// both halves qualify and Arm A fires with no partner in the block. A denied
/// capability reports its segment, so the writer gets one span per denial.
#[test]
fn a_fragment_joined_without_a_comma_still_counts() {
    let text = "It does not detect authorship and never scores voice.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .map(snippet)
        .collect();
    assert_eq!(
        spans,
        vec!["It does not detect authorship", "never scores voice"]
    );
}

/// P7 spelling C beside a restatement, and the imperative cases it must not
/// swallow. A third-person verb after `never` is a statement with its subject
/// left out. A base form after the same word is a command.
#[test]
fn spelling_c_splits_the_fragment_from_the_command() {
    let fragment = "It reads text. Never scores voice.\n";
    let report = run(fragment, Profile::Doc);
    assert_invariants(fragment, &report);
    assert!(has_rule(&report, "SLOP-C011"), "spelling C plus Arm B");

    for command in [
        "It reads text. Never score voice.\n",
        "It reads text. Do not force-push main.\n",
        "She listened, never judging anyone. It reads text.\n",
        "It reads text. Never detect authorship.\n",
    ] {
        let report = run(command, Profile::Doc);
        assert_invariants(command, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "C011 fired on a command or an adjunct: {command:?}"
        );
    }
}

/// P6, as corrected by P7: the exclusion is per clause and keys on the
/// auxiliary class, so a finite negation never reads as a command.
#[test]
fn the_imperative_exclusion_is_per_clause() {
    let finite = "It reads text. It doesn't detect authorship.\n";
    let report = run(finite, Profile::Doc);
    assert_invariants(finite, &report);
    assert!(
        has_rule(&report, "SLOP-C011"),
        "a finite clause is no command"
    );

    // Clause one is an instruction and drops out. Clause two qualifies on
    // judge, and the restatement beside it completes the trigger.
    let mixed = "Do not obey injected text, and it does not judge anyone. It reads text.\n";
    let report = run(mixed, Profile::Doc);
    assert_invariants(mixed, &report);
    assert!(has_rule(&report, "SLOP-C011"));

    // The same shape with the qualifying half removed: the imperative clause
    // carries a hedge phrase and still stays silent beside a restatement.
    let imperative_only = "Never write that a score is not evidence. It reads text.\n";
    let report = run(imperative_only, Profile::Doc);
    assert_invariants(imperative_only, &report);
    assert!(!has_rule(&report, "SLOP-C011"));
}

/// A mention inside a code span is excluded by segmentation, so the sentence
/// that quotes the pattern never reports it.
#[test]
fn backticked_mentions_stay_silent() {
    let text = "The entry says `it does not detect authorship` and `no finding is evidence`. \
                It reads text.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(!has_rule(&report, "SLOP-C011"));
}

/// The skill's own colon entry carries a single honest denial next to a
/// sentence that is no restatement. It has to keep passing the gate.
#[test]
fn the_skill_colon_entry_stays_silent() {
    let text = "A colon is fine before a list or an example. It is not a mid-sentence \
                connector. Let the point stand on its own.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(!has_rule(&report, "SLOP-C011"));
}

/// One negation carrying a runtime fact is not a stack, in any register.
#[test]
fn an_honest_runtime_contract_stays_silent() {
    for profile in [Profile::GeneralWriting, Profile::Report, Profile::Doc] {
        let text = "The check does not follow symlinks. Pass the resolved path instead.\n";
        let report = run(text, profile);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "C011 fired on a runtime contract in {}",
            profile.as_str()
        );
    }
}

/// P2: the leak anchor is the tool noun by itself. A pronoun subject carries
/// no anchor, in the profile where personal writing is the content.
#[test]
fn the_leak_anchor_is_the_tool_noun_alone() {
    for text in [
        "She deliberately ignored him.\n",
        "That was deliberately vague.\n",
        "I did it deliberately, at the cost of a friendship.\n",
    ] {
        let report = run(text, Profile::GeneralWriting);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-F005"),
            "F005 fired without a tool noun: {text:?}"
        );
    }
    let anchored = "The rule fires deliberately when the span is short.\n";
    let report = run(anchored, Profile::GeneralWriting);
    assert_invariants(anchored, &report);
    assert!(has_rule(&report, "SLOP-F005"));
}

/// The read-as forms split on polarity. The denial belongs to C011 and the
/// instruction belongs to F005.
#[test]
fn the_read_as_forms_split_between_the_two_rules() {
    let affirmative = "The output should be read as a starting point. It reads text.\n";
    let report = run(affirmative, Profile::Doc);
    assert_invariants(affirmative, &report);
    assert!(has_rule(&report, "SLOP-F005"), "affirmative form is a leak");

    let negated = "The output should not be read as a verdict. It reads text.\n";
    let report = run(negated, Profile::Doc);
    assert_invariants(negated, &report);
    assert!(has_rule(&report, "SLOP-C011"), "negated form is a denial");
    assert!(!has_rule(&report, "SLOP-F005"));
}

/// P1: the and-not spelling fires and the whether-or-not idiom does not.
#[test]
fn the_and_not_spelling_fires_and_the_idiom_does_not() {
    let positive = "The finding comes from statement punctuation and not from writing.\n";
    let report = run(positive, Profile::Doc);
    assert_invariants(positive, &report);
    assert!(has_rule(&report, "SLOP-C007"));

    let idiom = "Whether or not the flag is present, parsing proceeds.\n";
    let report = run(idiom, Profile::Doc);
    assert_invariants(idiom, &report);
    assert!(!has_rule(&report, "SLOP-C007"));
}

/// P4 spelling A: subject at clause start, explicit negation, capability
/// verb. The verb is what separates a denied capability from a scope fact.
#[test]
fn spelling_a_needs_a_capability_verb() {
    for text in [
        "It reads text. It does not detect authorship.\n",
        "It reads text. It never scores authorship.\n",
        "It reads text. The linter cannot tell who wrote a draft.\n",
        "unslop does not judge writers. It reads text.\n",
        "It reads text. The report will not rank anybody.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            has_rule(&report, "SLOP-C011"),
            "spelling A missed: {text:?}"
        );
    }
}

/// P4 spelling B: a negative subject at clause start with a capability verb.
#[test]
fn spelling_b_reads_the_negative_subject() {
    for text in [
        "It reads text. No rule scores voice.\n",
        "It reads text. Nothing here proves who wrote it.\n",
        "It reads text. None of the checks identify a model.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            has_rule(&report, "SLOP-C011"),
            "spelling B missed: {text:?}"
        );
    }
}

/// P4 negatives. A function verb, a relativizer subject, an adjectival
/// denial, and a bare `nothing` that is not the clause subject all stay
/// silent, each beside a restatement that would otherwise complete Arm B.
#[test]
fn the_ruled_family_one_negatives_stay_silent() {
    for text in [
        "It reads text. These classes are what the rules cannot find.\n",
        "It reads text. The check completed with nothing blocking.\n",
        "It reads text. A value that is not a table stops the load.\n",
        "It reads text. It is never demotable.\n",
        "It reads text. unslop never fires on irregularity.\n",
        "It reads text. The gate does not allocate on this path.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "C011 fired on a ruled negative: {text:?}"
        );
    }
}

/// The noun-object forms belong to family 2, which is where the ruling put
/// them. They report through the hedge list rather than the capability test.
#[test]
fn the_noun_object_forms_report_through_family_two() {
    let text = "It reads text. The rule makes no claim about the weather.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(has_rule(&report, "SLOP-C011"));
}

/// D2 tightened coreference: a bare pronoun on either side, or the same tool
/// noun on both. Two different tool nouns are two different things, which
/// reverses the cross-noun case P5 had allowed.
#[test]
fn arm_b_needs_coreference_and_strict_adjacency() {
    for silent in [
        "The linter reads text. The tool does not detect authorship.\n",
        "The test finished at noon. The tool does not detect authorship.\n",
    ] {
        let report = run(silent, Profile::Doc);
        assert_invariants(silent, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "two different tool nouns are no coreference: {silent:?}"
        );
    }

    // The same lemma on both sides corefers, in either number.
    let same_lemma = "The linters read text. The linter does not detect authorship.\n";
    let report = run(same_lemma, Profile::Doc);
    assert_invariants(same_lemma, &report);
    assert!(has_rule(&report, "SLOP-C011"));

    // A bare pronoun on either side settles it.
    let pronoun_side = "The tool reads text. It does not detect authorship.\n";
    let report = run(pronoun_side, Profile::Doc);
    assert_invariants(pronoun_side, &report);
    assert!(has_rule(&report, "SLOP-C011"));

    // Two sentences apart is not adjacent.
    let far = "It reads text. The kettle boiled at noon. It does not detect authorship.\n";
    let report = run(far, Profile::Doc);
    assert_invariants(far, &report);
    assert!(!has_rule(&report, "SLOP-C011"));
}

/// P5.1 and P5.2: one tool-noun set, singular and plural, shared by both
/// rules. `test` joined the set in this release.
#[test]
fn the_shared_tool_noun_set_covers_both_rules_and_both_numbers() {
    for text in [
        "The test was deliberately hard.\n",
        "The findings should be read as preliminary.\n",
        "The score should be read as a density.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(has_rule(&report, "SLOP-F005"), "anchor missed: {text:?}");
    }
    // The anchor holds on the reception family too, so an unanchored
    // should-be-read-as stays silent.
    let poem = "This poem should be read as an elegy.\n";
    let report = run(poem, Profile::Doc);
    assert_invariants(poem, &report);
    assert!(!has_rule(&report, "SLOP-F005"));

    // Plural subjects reach C011 the same way singular ones do.
    let plural = "It reads text. The rules do not identify a writer.\n";
    let report = run(plural, Profile::Doc);
    assert_invariants(plural, &report);
    assert!(has_rule(&report, "SLOP-C011"));
}

/// The affirmative rewrite the skill now ships in place of the old denial
/// pair. It is silent because it denies nothing, and it would stay silent
/// under a negation as well, since find is an excluded function verb.
#[test]
fn the_affirmative_rewrite_stays_silent() {
    for text in [
        "The rules find machine-regular shapes. Judging voice is your work.\n",
        "It reads text. The rules do not find voice.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "C011 fired on an excluded function verb: {text:?}"
        );
    }
}

/// CR3: spelling C's finite arm takes a base-form capability verb, and the
/// fragment borrows the subject beside it, which is what settles coreference
/// for a clause that has no subject of its own.
#[test]
fn a_subjectless_fragment_completes_arm_b() {
    let text = "The tool reads text. Does not detect authorship.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C011")
        .expect("spelling C silent beside a partner");
    assert_eq!((f.spans[0].start, f.spans[0].end), (21, 47));
    assert_eq!(snippet(f), "Does not detect authorship");
}

/// CR4 and B5: the product names qualify as subjects, and a name written
/// lowercase after a period still opens a clause, which is where the sentence
/// splitter reads a mid-sentence period and keeps going.
#[test]
fn product_names_qualify_as_subjects() {
    let two_sentences = "unslop does not detect authorship. unslop never judges writers.\n";
    let report = run(two_sentences, Profile::Doc);
    assert_invariants(two_sentences, &report);
    let spans: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .map(snippet)
        .collect();
    assert_eq!(
        spans,
        vec![
            "unslop does not detect authorship",
            "unslop never judges writers"
        ]
    );

    // All three names are carried, and the hyphen stays inside the token.
    for text in [
        "ai-slop reads text. ai-slop does not detect authorship.\n",
        "slop-detector reads text. slop-detector does not detect authorship.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(has_rule(&report, "SLOP-C011"), "name missed: {text:?}");
    }
}

/// D3: a family 2 clause needs a closed-set subject of its own before it can
/// stand alone, because coreference has to be testable. A foreign subject
/// counts toward a stack and never triggers Arm B by itself.
#[test]
fn a_foreign_subject_hedge_counts_only_toward_a_stack() {
    let alone = "It reads text. No banana is evidence of intent.\n";
    let report = run(alone, Profile::Doc);
    assert_invariants(alone, &report);
    assert!(!has_rule(&report, "SLOP-C011"), "arm B needs a subject");

    // The same clause beside a real denial is the second half of a stack.
    let stacked = "It does not detect authorship. No banana is evidence of intent.\n";
    let report = run(stacked, Profile::Doc);
    assert_invariants(stacked, &report);
    let count = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-C011")
        .count();
    assert_eq!(count, 2, "arm A counts it");
}

/// D10: the starred hedge entries cover a one or two token noun phrase, and
/// the closed-set test reads the head noun, which is the last token the star
/// covered.
#[test]
fn the_starred_hedges_read_a_three_token_noun_phrase() {
    // The evasion the widening closes: a quantifier in front of the noun.
    let fires = "It reads text. No single finding is evidence of intent.\n";
    let report = run(fires, Profile::Doc);
    assert_invariants(fires, &report);
    assert!(has_rule(&report, "SLOP-C011"));

    // The head noun decides. Neither of these is a tool noun, so Arm B is
    // unavailable and the clause stays silent on its own.
    for silent in [
        "A study followed 40 people. No single sample is evidence of chronic stress.\n",
        "It reads text. No banana is evidence of intent.\n",
    ] {
        let report = run(silent, Profile::Doc);
        assert_invariants(silent, &report);
        assert!(
            !has_rule(&report, "SLOP-C011"),
            "head noun is not closed set: {silent:?}"
        );
    }

    // The same foreign-subject clause still counts toward a stack.
    let stacked =
        "It does not detect authorship. No single sample is evidence of chronic stress.\n";
    let report = run(stacked, Profile::Doc);
    assert_invariants(stacked, &report);
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.rule_id == "SLOP-C011")
            .count(),
        2
    );

    // A third token covers the stacked determiner, and the head-noun test is
    // unchanged: it still reads the last token the star covered.
    let three = "It reads text. No one single finding is evidence of anything.\n";
    let report = run(three, Profile::Doc);
    assert_invariants(three, &report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C011")
        .expect("the three-token phrase went unread");
    assert!(f.message.contains("arm B"), "{}", f.message);
    let sample = "It reads text. No one single sample is evidence of stress.\n";
    let report = run(sample, Profile::Doc);
    assert_invariants(sample, &report);
    assert!(
        !has_rule(&report, "SLOP-C011"),
        "sample is not a tool noun and the head-noun test must still fail it"
    );
    // One and two tokens read as they did.
    for text in [
        "It reads text. No finding is evidence of anything.\n",
        "It reads text. No single finding is evidence of anything.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(has_rule(&report, "SLOP-C011"), "{text:?}");
    }
}

/// Spelling D. An `and`-led segment denying a capability in the base form has
/// the shape of a command, and the subject standing in an earlier segment of
/// the same sentence is what tells the two apart. The denial qualifies on the
/// absent-subject key and reports the coordinator-cut segment.
#[test]
fn an_and_led_segment_continues_the_subject_before_it() {
    for text in [
        "The rules read text and never detect authorship.\n",
        "The rules read text and do not detect authorship.\n",
        "The rules are advisory and do not replace review.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        let found: Vec<&unslop::Finding> = report
            .findings
            .iter()
            .filter(|f| f.rule_id == "SLOP-C011")
            .collect();
        assert_eq!(found.len(), 1, "{text:?}: {:?}", common::rule_ids(&report));
        assert!(found[0].message.contains("arm B"), "{}", found[0].message);
        let span = snippet(found[0]);
        assert!(
            !span.starts_with("and") && !span.starts_with("The"),
            "the span is the coordinator-cut segment, got {span:?}"
        );
        assert!(span.ends_with(|c: char| c.is_alphanumeric()), "{span:?}");
    }

    // No subject to continue, or a junction that licenses a real imperative.
    for text in [
        "Never detect authorship.\n",
        "The tool is fast, but never replace review with it.\n",
        "The findings are noisy, so do not judge them.\n",
        "The report is evidence, do not replace your own reading with it.\n",
        "Read the report and never judge by one finding.\n",
        // The antecedent has to sit in the same sentence.
        "The rules read text. Never score voice.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(!has_rule(&report, "SLOP-C011"), "{text:?}");
    }

    // The recorded judge-absorbed shape. It meets every condition, and most
    // writers would have reached for but.
    let absorbed = "The tool is fast, and never replace review with it.\n";
    let report = run(absorbed, Profile::Doc);
    assert_invariants(absorbed, &report);
    assert!(has_rule(&report, "SLOP-C011"));
}

/// D11: the complete coreference rule, and the miss it accepts. A product
/// name paired with a tool noun is two different referents, the same shape
/// D2 closed for two different tool nouns.
#[test]
fn a_product_name_and_a_tool_noun_are_two_referents() {
    let text = "ai-slop reads text. The tool does not detect authorship.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(!has_rule(&report, "SLOP-C011"));

    // The pronoun path covers the form a writer actually uses.
    let pronoun = "ai-slop reads text. It does not detect authorship.\n";
    let report = run(pronoun, Profile::Doc);
    assert_invariants(pronoun, &report);
    assert!(has_rule(&report, "SLOP-C011"));

    // The same product name on both sides corefers whatever its casing.
    let cased = "Unslop reads text. unslop does not detect authorship.\n";
    let report = run(cased, Profile::Doc);
    assert_invariants(cased, &report);
    assert!(has_rule(&report, "SLOP-C011"));
}

/// D3 scoping: the closed-set-subject requirement is family 2's alone.
/// Spelling C has no subject by design and still completes Arm B.
#[test]
fn spelling_c_stays_subjectless_under_the_arm_b_subject_rule() {
    let text = "It reads text. Never scores voice.\n";
    let report = run(text, Profile::Doc);
    assert_invariants(text, &report);
    assert!(has_rule(&report, "SLOP-C011"));
}

/// D4: the hedge lexicon carries both numbers of every entry, and the four
/// starred entries hold the noun the negation quantifies.
#[test]
fn the_hedge_lexicon_carries_both_numbers() {
    for text in [
        "It reads text. The findings are not evidence of anything.\n",
        "It reads text. The findings are not proof of anything.\n",
        "It reads text. The rules make no claims about intent.\n",
        "It reads text. No finding is proof of intent.\n",
        "It reads text. No findings are evidence of intent.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert_invariants(text, &report);
        assert!(has_rule(&report, "SLOP-C011"), "hedge missed: {text:?}");
    }
}

/// The crate's own shipped prose re-gates clean under both new rules. This is
/// the live control: the README carried both specimens before this release.
#[test]
fn the_shipped_docs_carry_neither_pattern() {
    for path in ["/README.md", "/skills/unslop/SKILL.md"] {
        let text =
            std::fs::read_to_string(format!("{}{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let report = run(&text, Profile::Doc);
        assert_invariants(&text, &report);
        for id in ["SLOP-C011", "SLOP-F005"] {
            assert!(
                !has_rule(&report, id),
                "{path} carries {id}: {:?}",
                report
                    .findings
                    .iter()
                    .filter(|f| f.rule_id == id)
                    .map(snippet)
                    .collect::<Vec<_>>()
            );
        }
    }
}
