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
        Profile::Essay,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
    ] {
        assert_eq!(rule("SLOP-M002").stance(p), Stance::Off);
    }

    // The first-person split: plain authorial first person is off in the
    // voice profiles, relaxes in report, applies in doc. Process narration
    // fires in every profile.
    for p in [
        Profile::Essay,
        Profile::BlogPost,
        Profile::Email,
        Profile::SocialPost,
    ] {
        assert_eq!(rule("SLOP-F001").stance(p), Stance::Off);
    }
    assert_eq!(rule("SLOP-F001").stance(Profile::Report), Stance::Relax);
    assert_eq!(rule("SLOP-F001").stance(Profile::Doc), Stance::Apply);
    for p in Profile::ALL {
        assert_eq!(rule("SLOP-F004").stance(p), Stance::Apply);
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
        Profile::Essay,
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
    for p in [Profile::Essay, Profile::BlogPost, Profile::SocialPost] {
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
    assert_eq!(rule("SLOP-E002").stance(Profile::Essay), Stance::Apply);

    // Metaphor nouns: off where the words are usually literal.
    assert_eq!(rule("SLOP-A008").stance(Profile::Doc), Stance::Off);
    assert_eq!(rule("SLOP-A008").stance(Profile::Report), Stance::Relax);
    for p in [
        Profile::Essay,
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
    assert_eq!(rule("SLOP-X003").stance(Profile::Essay), Stance::Off);

    // The License section is a doc-profile contract.
    assert_eq!(rule("SLOP-K005").stance(Profile::Doc), Stance::Apply);
    assert_eq!(rule("SLOP-K005").stance(Profile::Essay), Stance::Off);
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
