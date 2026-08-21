//! Section 12.12: for every rule x profile pair the resolved stance matches
//! the policy package, plus the named stance points the rulings fixed.

use unslop::policy::{self, Tier};
use unslop::{Profile, Stance};

fn expected_stance(rule_tbl: &toml::Value, profile: &str) -> Stance {
    let profiles = rule_tbl
        .get("profiles")
        .and_then(|v| v.as_table())
        .expect("profiles table");
    let entry = profiles.get(profile).or_else(|| profiles.get("default"));
    let s = match entry {
        None => "apply",
        Some(toml::Value::String(s)) => s.as_str(),
        Some(_) => panic!("bad stance value"),
    };
    match s {
        "apply" => Stance::Apply,
        "relax" => Stance::Relax,
        "off" => Stance::Off,
        other => panic!("unknown stance {other}"),
    }
}

#[test]
fn full_matrix_matches_the_package() {
    let pkg = policy::load().unwrap();
    let root: toml::Value = toml::from_str(policy::POLICY_TOML).unwrap();
    let raw_rules = root.get("rule").and_then(|v| v.as_array()).unwrap();
    assert_eq!(pkg.rules.len(), raw_rules.len());

    for (rule, raw) in pkg.rules.iter().zip(raw_rules) {
        assert_eq!(
            rule.id,
            raw.get("id").and_then(|v| v.as_str()).unwrap(),
            "rule order drift"
        );
        for profile in Profile::ALL {
            let got = rule.stance(profile);
            let want = expected_stance(raw, profile.as_str());
            assert_eq!(got, want, "{} x {}", rule.id, profile.as_str());
        }
    }
}

#[test]
fn known_matrix_points() {
    let pkg = policy::load().unwrap();
    let rule = |id: &str| pkg.rule_by_id(id).unwrap();

    // Semicolons block in doc, report to the writer in report, and say
    // nothing in the four profiles where the mark is a writer's choice.
    assert_eq!(rule("SLOP-M002").stance(Profile::Doc), Stance::Apply);
    assert_eq!(rule("SLOP-M002").stance(Profile::Report), Stance::Relax);
    for p in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
    ] {
        assert_eq!(rule("SLOP-M002").stance(p), Stance::Off);
    }

    // The first-person split: plain authorial first person is off in the
    // voice profiles, relaxes in report, applies in doc. Process narration
    // fires in every profile and relaxes in comment.
    for p in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
        Profile::Comment,
    ] {
        assert_eq!(rule("SLOP-F001").stance(p), Stance::Off);
    }
    assert_eq!(rule("SLOP-F001").stance(Profile::Report), Stance::Relax);
    assert_eq!(rule("SLOP-F001").stance(Profile::Doc), Stance::Apply);
    for p in Profile::ALL {
        let want = if p == Profile::Comment {
            Stance::Relax
        } else {
            Stance::Apply
        };
        assert_eq!(rule("SLOP-F004").stance(p), want);
    }

    // Emoji relax to candidate in social-post only.
    assert_eq!(rule("SLOP-M006").stance(Profile::SocialPost), Stance::Relax);
    assert_eq!(rule("SLOP-M006").stance(Profile::BlogPost), Stance::Apply);

    // The courtesy splits: the assistant half stays on in email, the half
    // addressed to a person the writer knows is off there.
    for p in Profile::ALL {
        assert_eq!(rule("SLOP-S001").stance(p), Stance::Apply);
        assert_eq!(rule("SLOP-S003").stance(p), Stance::Apply);
        assert_eq!(rule("SLOP-V003").stance(p), Stance::Apply);
    }
    assert_eq!(rule("SLOP-S004").stance(Profile::Email), Stance::Off);
    assert_eq!(rule("SLOP-S004").stance(Profile::SocialPost), Stance::Off);
    assert_eq!(rule("SLOP-S004").stance(Profile::Doc), Stance::Apply);
    assert_eq!(rule("SLOP-S005").stance(Profile::Email), Stance::Off);
    assert_eq!(rule("SLOP-S005").stance(Profile::Doc), Stance::Apply);
    assert_eq!(rule("SLOP-V006").stance(Profile::Email), Stance::Off);
    assert_eq!(rule("SLOP-V006").stance(Profile::Doc), Stance::Apply);

    // The style rules the second round added.
    for p in Profile::ALL {
        assert_eq!(rule("SLOP-E004").stance(p), Stance::Apply);
        assert_eq!(rule("SLOP-E005").stance(p), Stance::Apply);
    }
    assert_eq!(rule("SLOP-M008").stance(Profile::Doc), Stance::Apply);
    for p in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::Report,
        Profile::SocialPost,
    ] {
        assert_eq!(rule("SLOP-M008").stance(p), Stance::Off);
    }

    // The hedge split: stacks fire everywhere, single softeners follow the
    // profile's tolerance for a writer's own voice.
    for p in Profile::ALL {
        assert_eq!(rule("SLOP-I005").stance(p), Stance::Apply);
    }
    for p in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::SocialPost,
    ] {
        assert_eq!(rule("SLOP-I006").stance(p), Stance::Off);
    }
    assert_eq!(rule("SLOP-I006").stance(Profile::Email), Stance::Relax);
    assert_eq!(rule("SLOP-I006").stance(Profile::Report), Stance::Apply);
    assert_eq!(rule("SLOP-I006").stance(Profile::Doc), Stance::Apply);

    // The ornamental split: A001 blocks, A010 reports, both everywhere.
    for p in Profile::ALL {
        assert_eq!(rule("SLOP-A001").stance(p), Stance::Apply);
        assert_eq!(rule("SLOP-A010").stance(p), Stance::Apply);
    }
    assert_eq!(rule("SLOP-E002").stance(Profile::Email), Stance::Relax);
    assert_eq!(rule("SLOP-E002").stance(Profile::SocialPost), Stance::Relax);
    assert_eq!(
        rule("SLOP-E002").stance(Profile::GeneralWriting),
        Stance::Apply
    );

    // Metaphor nouns: off where the words are usually literal.
    assert_eq!(rule("SLOP-A008").stance(Profile::Doc), Stance::Off);
    assert_eq!(rule("SLOP-A008").stance(Profile::Report), Stance::Relax);
    for p in [
        Profile::GeneralWriting,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
    ] {
        assert_eq!(rule("SLOP-A008").stance(p), Stance::Apply);
    }

    // Structure rules: the heading skeleton is hardest in blog-post and off
    // where structure is expected or absent.
    assert_eq!(rule("SLOP-X001").stance(Profile::BlogPost), Stance::Apply);
    for p in [Profile::Email, Profile::Report, Profile::SocialPost] {
        assert_eq!(rule("SLOP-X001").stance(p), Stance::Off);
    }
    assert_eq!(rule("SLOP-X004").stance(Profile::Email), Stance::Off);
    assert_eq!(rule("SLOP-X003").stance(Profile::Email), Stance::Apply);
    assert_eq!(
        rule("SLOP-X003").stance(Profile::GeneralWriting),
        Stance::Off
    );

    // The License section is a doc-profile contract.
    assert_eq!(rule("SLOP-K005").stance(Profile::Doc), Stance::Apply);
    assert_eq!(
        rule("SLOP-K005").stance(Profile::GeneralWriting),
        Stance::Off
    );
}

/// The comment profile in full: eleven rules move and nothing else does.
/// A thread reply is stricter than email wherever the assistant register is
/// in play and softer wherever the writer's own voice is.
#[test]
fn the_comment_profile_moves_eleven_rules() {
    let pkg = policy::load().unwrap();
    let rule = |id: &str| pkg.rule_by_id(id).unwrap();
    let c = Profile::Comment;

    for id in [
        "SLOP-A008",
        "SLOP-M006",
        "SLOP-S005",
        "SLOP-E002",
        "SLOP-F002",
        "SLOP-F004",
    ] {
        assert_eq!(rule(id).stance(c), Stance::Relax, "{id} relaxes in comment");
    }
    for id in ["SLOP-I006", "SLOP-D002", "SLOP-X001", "SLOP-X004"] {
        assert_eq!(rule(id).stance(c), Stance::Off, "{id} is off in comment");
    }
    assert_eq!(rule("SLOP-X003").stance(c), Stance::Apply);

    // Every other rule reads its default in comment, so the eleven above are
    // the whole of the profile. Anything else moving is drift. I006 is off in
    // general-writing too and so does not show up as a difference.
    let moved: Vec<&str> = pkg
        .rules
        .iter()
        .filter(|r| r.stance(c) != r.stance(Profile::GeneralWriting))
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(
        moved,
        [
            "SLOP-A008",
            "SLOP-M006",
            "SLOP-S005",
            "SLOP-E002",
            "SLOP-F002",
            "SLOP-F004",
            "SLOP-D002",
            "SLOP-X001",
            "SLOP-X003",
            "SLOP-X004",
        ]
    );

    // The five strictest rules carry no comment relaxation at all. S004 and
    // V006 are the deliberate divergence from email, which turns both off.
    for id in [
        "SLOP-S003",
        "SLOP-V003",
        "SLOP-V002",
        "SLOP-S004",
        "SLOP-V006",
    ] {
        assert_eq!(
            rule(id).stance(c),
            Stance::Apply,
            "{id} must stay at full strength in comment"
        );
    }
    assert_eq!(rule("SLOP-S004").stance(Profile::Email), Stance::Off);
    assert_eq!(rule("SLOP-V006").stance(Profile::Email), Stance::Off);

    // Rule 3 applies: a sign-off in a thread is letter furniture.
    let core = &pkg.profiles[c.index()].core_rules;
    assert_eq!(pkg.profiles[c.index()].name, "comment");
    assert_eq!(core.get("rule1").unwrap(), "off");
    assert_eq!(core.get("rule2").unwrap(), "invert");
    assert_eq!(core.get("rule3").unwrap(), "apply");
    assert_eq!(core.get("rule4").unwrap(), "apply");

    // The word cap is the comment key on X003, not the email one.
    let caps = rule("SLOP-X003")
        .params
        .as_table()
        .and_then(|t| t.get("max_words"))
        .and_then(|v| v.as_table())
        .unwrap();
    assert_eq!(caps.get("comment").and_then(|v| v.as_integer()), Some(400));
    assert_eq!(caps.get("email").and_then(|v| v.as_integer()), Some(600));
    assert_eq!(
        rule("SLOP-X003").lifecycle,
        policy::Lifecycle::Experimental,
        "the cap reports and never gates"
    );
}

/// essay is gone. general-writing is the name, there is no alias, and the
/// profile keeps its index so stored stances do not shift.
#[test]
fn the_renamed_profile_answers_to_one_name() {
    assert_eq!(Profile::GeneralWriting.as_str(), "general-writing");
    assert_eq!(Profile::from_str("essay"), None);
    assert_eq!(
        Profile::from_str("general-writing"),
        Some(Profile::GeneralWriting)
    );
    assert_eq!(Profile::GeneralWriting.index(), 0);
    assert_eq!(Profile::ALL.len(), 7);
    let pkg = policy::load().unwrap();
    assert_eq!(pkg.profile_names[0], "general-writing");
    assert!(!pkg.profile_names.iter().any(|n| n == "essay"));
    assert!(!policy::POLICY_TOML.contains("[profile.essay]"));
}

#[test]
fn tier_counts_are_pinned() {
    let pkg = policy::load().unwrap();
    assert_eq!(pkg.rules.len(), 93);
    let count = |t: Tier| pkg.rules.iter().filter(|r| r.tier == t).count();
    assert_eq!(count(Tier::Violation), 22);
    assert_eq!(count(Tier::Candidate), 64);
    assert_eq!(count(Tier::CoverageHint), 7);
}

#[test]
fn every_profile_is_covered_by_the_package() {
    let pkg = policy::load().unwrap();
    assert_eq!(pkg.profile_names.len(), Profile::ALL.len());
    for (i, p) in Profile::ALL.iter().enumerate() {
        assert_eq!(pkg.profile_names[i], p.as_str());
        assert_eq!(pkg.profiles[i].name, p.as_str());
        assert_eq!(pkg.profiles[i].core_rules.len(), 4);
    }
}
