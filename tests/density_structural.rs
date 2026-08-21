//! Section 12.14: density and structural threshold fixtures fire exactly on
//! the declared side of each boundary.

mod common;

use common::{has_rule, run};
use unslop::Profile;

fn filler_words(n: usize) -> String {
    (0..n)
        .map(|i| format!("w{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn lexicon_density_boundary_two_vs_three_per_500_words() {
    // Around 500 words with exactly 2 vs 3 counted lexicon hits.
    let base = filler_words(490);
    let two = format!("{base} delve tapestry end.");
    let report = run(&two, Profile::GeneralWriting);
    assert!(!has_rule(&report, "SLOP-D001"), "2 hits fired D001");

    let three = format!("{base} delve tapestry testament end.");
    let report = run(&three, Profile::GeneralWriting);
    assert!(has_rule(&report, "SLOP-D001"), "3 hits missed D001");
}

#[test]
fn opener_density_three_per_document() {
    let two = "Moreover, one.\n\nFurthermore, two.\n\nPlain third.\n";
    let report = run(two, Profile::GeneralWriting);
    assert!(!has_rule(&report, "SLOP-D004"));

    let three = "Moreover, one.\n\nFurthermore, two.\n\nAdditionally, three.\n";
    let report = run(three, Profile::GeneralWriting);
    assert!(has_rule(&report, "SLOP-D004"));
}

#[test]
fn over_structured_short_doc() {
    let doc = "# A\n\ntext\n\n## B\n\ntext\n\n## C\n\ntext\n\n## D\n\ntext\n";
    let report = run(doc, Profile::Doc);
    assert!(has_rule(&report, "SLOP-X004"));

    let long = format!(
        "# A\n\n{}\n\n## B\n\ntext\n\n## C\n\ntext\n\n## D\n\ntext\n",
        filler_words(400)
    );
    let report = run(&long, Profile::Doc);
    assert!(!has_rule(&report, "SLOP-X004"));
}

/// The comment cap is 400 words where email's is 600, and both report to a
/// judge without gating.
#[test]
fn the_comment_word_cap_sits_below_the_email_one() {
    let five_hundred = format!("{}\n", filler_words(500));
    let report = run(&five_hundred, Profile::Comment);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-X003")
        .expect("the 400-word cap went unreported in comment");
    assert_eq!(f.lifecycle, "experimental");
    assert_eq!(report.exit_code(), 0, "an experimental rule never gates");
    assert!(!has_rule(&run(&five_hundred, Profile::Email), "SLOP-X003"));

    let three_hundred = format!("{}\n", filler_words(300));
    assert!(!has_rule(
        &run(&three_hundred, Profile::Comment),
        "SLOP-X003"
    ));

    // The structure rules are off in comment, and so is the link-bullet one.
    let headed = "# A\n\ntext\n\n## B\n\ntext\n\n## C\n\ntext\n\n## D\n\ntext\n";
    let report = run(headed, Profile::Comment);
    assert!(!has_rule(&report, "SLOP-X004"));
    let skeleton =
        "# Title\n\n## Introduction\n\nx\n\n## Key Takeaways\n\nx\n\n## Final Thoughts\n\nx\n";
    assert!(!has_rule(&run(skeleton, Profile::Comment), "SLOP-X001"));
}

#[test]
fn boilerplate_skeleton_headings() {
    let doc =
        "# Title\n\n## Introduction\n\nx\n\n## Key Takeaways\n\nx\n\n## Final Thoughts\n\nx\n";
    let report = run(doc, Profile::BlogPost);
    assert!(has_rule(&report, "SLOP-X001"));
    // Relaxed in general-writing: still reported, never gating.
    let report = run(doc, Profile::GeneralWriting);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-X001")
        .expect("X001 still reports in general-writing");
    assert_eq!(f.lifecycle, "advisory");
    // Off where structure is expected or absent.
    let report = run(doc, Profile::Report);
    assert!(!has_rule(&report, "SLOP-X001"));
}

#[test]
fn bold_label_list_needs_three_items() {
    let two = "- **Fast**: quick\n- **Safe**: sound\n";
    let report = run(two, Profile::Doc);
    assert!(!has_rule(&report, "SLOP-E003"));

    let three = "- **Fast**: quick\n- **Safe**: sound\n- **Clean**: neat\n";
    let report = run(three, Profile::Doc);
    assert!(has_rule(&report, "SLOP-E003"));
}

#[test]
fn emphasis_staged_contrast() {
    let doc = "The parser *does* accept this input, but the writer rejects it.\n";
    let report = run(doc, Profile::Doc);
    assert!(has_rule(&report, "SLOP-E001"));

    let plain = "The parser *quickly* accepts this input, but slowly.\n";
    let report = run(plain, Profile::Doc);
    assert!(!has_rule(&report, "SLOP-E001"));
}
