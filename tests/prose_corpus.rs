//! Golden corpus. Three short documents written for this crate, each
//! carrying patterns on purpose, each pinned to the exact set of rules it
//! raises. A change in any rule that moves one of these lists has to be
//! looked at and either accepted here or fixed there.

mod common;

use common::{assert_invariants, run};
use unslop::Profile;

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/prose/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

fn ids(report: &unslop::Report) -> Vec<&str> {
    report.findings.iter().map(|f| f.rule_id.as_str()).collect()
}

#[test]
fn essay_excerpt_is_stable() {
    let text = fixture("essay-excerpt.md");
    let report = run(&text, Profile::Essay);
    assert_invariants(&text, &report);
    assert_eq!(
        ids(&report),
        vec![
            // puffery in the opening claim
            "SLOP-A006",
            // the false range and the triplet shape that share its span
            "SLOP-C010",
            "SLOP-C005",
            // the participial tail closing its block
            "SLOP-O005",
            // the stock ending
            "SLOP-O008",
        ]
    );
    // Voice is content here: first person and the plain past tense pass.
    assert!(!common::has_rule(&report, "SLOP-F001"));
    assert_eq!(report.result_state, "candidates_present");
}

#[test]
fn email_is_stable() {
    let text = fixture("email.md");
    let report = run(&text, Profile::Email);
    assert_invariants(&text, &report);
    assert_eq!(
        ids(&report),
        vec![
            // the contrast-rate instrument, which never gates
            "SLOP-C009",
            // the assistant register, which email keeps
            "SLOP-S003",
            "SLOP-C003",
            // process narration, which fires in every profile
            "SLOP-F004",
            "SLOP-V003",
        ]
    );
    // The human half of the courtesy set is correspondence structure here.
    for id in ["SLOP-S004", "SLOP-S005", "SLOP-V006"] {
        assert!(!common::has_rule(&report, id), "{id} fired in an email");
    }
}

#[test]
fn blog_fragment_is_stable() {
    let text = fixture("blog-fragment.md");
    let report = run(&text, Profile::BlogPost);
    assert_invariants(&text, &report);
    assert_eq!(
        ids(&report),
        vec![
            "SLOP-D001",
            "SLOP-E004",
            "SLOP-E005",
            "SLOP-I002",
            // empowers and unlock report, full potential and seamless block:
            // the ornamental set splits on whether a plain sense survives.
            "SLOP-A010",
            "SLOP-A010",
            "SLOP-A001",
            "SLOP-M006",
            "SLOP-E003",
            "SLOP-A010",
            "SLOP-O003",
            "SLOP-T002",
            "SLOP-A001",
        ]
    );
    assert_eq!(report.result_state, "violations_present");
}

/// Every fixture holds the span and segmentation invariants under every
/// profile, not only its own.
#[test]
fn fixtures_hold_invariants_under_every_profile() {
    for name in ["essay-excerpt.md", "email.md", "blog-fragment.md"] {
        let text = fixture(name);
        for profile in Profile::ALL {
            let report = run(&text, profile);
            assert_invariants(&text, &report);
        }
    }
}
