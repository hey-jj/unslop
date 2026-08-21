//! Section 12.15: the waiver-authority floor. An agent-signed waiver on a
//! human-only rule, on SLOP-J001, or on a fail-closed state is rejected
//! under every configuration. Demoting a violation-tier rule is a usage
//! error.

mod common;

use unslop::waiver::{Approval, Waiver, WaiverSpan};
use unslop::{analyze, AnalysisError, Config, Profile, VerifyOutcome, WaiverAuthority};

fn approval_with(waiver: Waiver, payload: &[u8]) -> Approval {
    Approval {
        document_sha256: unslop::input::sha256_hex(payload),
        policy_digest: unslop::policy_digest(),
        profile: "doc".to_string(),
        approved_at: None,
        approver: Some("test".to_string()),
        approver_kind: Some("human".to_string()),
        demote: vec![],
        waivers: vec![waiver],
    }
}

fn agent_waiver(rule_id: &str) -> Waiver {
    Waiver {
        rule_id: rule_id.to_string(),
        span: Some(WaiverSpan { start: 0, end: 10 }),
        reason: Some("test".to_string()),
        signer_kind: Some("orchestrator-agent".to_string()),
        expires: Some("2999-01-01T00:00:00Z".to_string()),
    }
}

#[test]
fn verify_rejects_agent_waivers_on_the_floor() {
    let payload = b"anything";
    for rule_id in ["SLOP-A001", "SLOP-A002", "SLOP-J001"] {
        let approval = approval_with(agent_waiver(rule_id), payload);
        match unslop::verify(payload, &approval, 0) {
            VerifyOutcome::Mismatch(problems) => {
                assert!(
                    problems.iter().any(|p| p.contains(rule_id)),
                    "{rule_id}: floor problem not reported: {problems:?}"
                );
            }
            VerifyOutcome::Verified => panic!("{rule_id}: floor waiver accepted"),
        }
    }
}

#[test]
fn verify_rejects_waivers_on_fail_closed_states_from_any_signer() {
    let payload = b"anything";
    for rule_id in ["instrumentation_error", "unsupported_input"] {
        let mut w = agent_waiver(rule_id);
        w.signer_kind = Some("human".to_string());
        let approval = approval_with(w, payload);
        assert!(
            matches!(
                unslop::verify(payload, &approval, 0),
                VerifyOutcome::Mismatch(_)
            ),
            "{rule_id}: waiver on fail-closed state accepted"
        );
    }
}

#[test]
fn verify_accepts_a_clean_document_with_a_matching_hash() {
    // A genuinely clean document (no blocking findings under its profile) with
    // a matching hash and digest verifies. `verify` re-runs the analysis, so
    // the document must actually be clean, not merely byte-match the approval.
    let payload = b"Reads a file and returns its bytes.";
    let approval = Approval {
        document_sha256: unslop::input::sha256_hex(payload),
        policy_digest: unslop::policy_digest(),
        profile: "general-writing".to_string(),
        approved_at: None,
        approver: Some("test".to_string()),
        approver_kind: Some("human".to_string()),
        demote: vec![],
        waivers: vec![],
    };
    assert_eq!(
        unslop::verify(payload, &approval, 0),
        VerifyOutcome::Verified
    );
}

#[test]
fn verify_fails_on_hash_digest_or_expiry_mismatch() {
    let payload = b"anything";
    let mut approval = approval_with(agent_waiver("SLOP-S002"), payload);
    approval.waivers.clear();
    assert!(matches!(
        unslop::verify(b"mutated payload", &approval, 0),
        VerifyOutcome::Mismatch(_)
    ));
    let mut bad_digest = approval_with(agent_waiver("SLOP-S002"), payload);
    bad_digest.waivers.clear();
    bad_digest.policy_digest = "sha256:0000".to_string();
    assert!(matches!(
        unslop::verify(payload, &bad_digest, 0),
        VerifyOutcome::Mismatch(_)
    ));
    let mut expired = approval_with(
        Waiver {
            expires: Some("2020-01-01T00:00:00Z".to_string()),
            signer_kind: Some("human".to_string()),
            ..agent_waiver("SLOP-S002")
        },
        payload,
    );
    expired.profile = "doc".to_string();
    assert!(matches!(
        unslop::verify(payload, &expired, 4_102_444_800),
        VerifyOutcome::Mismatch(_)
    ));
}

// --- verify re-runs analysis: an approval must be EARNED, not byte-matched.

fn approval(profile: &str, payload: &[u8], waivers: Vec<Waiver>) -> Approval {
    Approval {
        document_sha256: unslop::input::sha256_hex(payload),
        policy_digest: unslop::policy_digest(),
        profile: profile.to_string(),
        approved_at: None,
        approver: Some("test".to_string()),
        approver_kind: Some("human".to_string()),
        demote: vec![],
        waivers,
    }
}

fn waiver(
    rule_id: &str,
    signer: Option<&str>,
    span: Option<(usize, usize)>,
    expires: Option<&str>,
) -> Waiver {
    Waiver {
        rule_id: rule_id.to_string(),
        span: span.map(|(start, end)| WaiverSpan { start, end }),
        reason: Some("test".to_string()),
        signer_kind: signer.map(|s| s.to_string()),
        expires: expires.map(|s| s.to_string()),
    }
}

fn rejects(outcome: VerifyOutcome) -> Vec<String> {
    match outcome {
        VerifyOutcome::Mismatch(problems) => problems,
        VerifyOutcome::Verified => panic!("expected rejection, got Verified"),
    }
}

// Hole 1: an empty-waiver approval whose hash matches a dirty document must be
// rejected, because verify now re-runs the linter.
#[test]
fn verify_rejects_empty_waiver_over_a_dirty_document() {
    let payload = b"We delve into this."; // SLOP-A001 violation, blocking.
    let approval = approval("general-writing", payload, vec![]);
    let problems = rejects(unslop::verify(payload, &approval, 1_000_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-A001")),
        "unwaived A001 not reported: {problems:?}"
    );
}

// Hole 2: analysis is bound to the approval's profile. The same bytes verify
// under a lax profile and are rejected under the stricter profile they should
// have been linted under.
#[test]
fn verify_binds_analysis_to_the_approval_profile() {
    // First person is a finding in doc and content in general-writing.
    let payload = b"I think this is the better road.";
    let strict = approval("doc", payload, vec![]);
    let problems = rejects(unslop::verify(payload, &strict, 1_000_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-F001")),
        "strict profile not applied: {problems:?}"
    );
    let lax = approval("general-writing", payload, vec![]);
    assert_eq!(
        unslop::verify(payload, &lax, 1_000_000_000),
        VerifyOutcome::Verified,
        "lax profile should verify the same bytes"
    );
}

// Hole 3: a waiver is human-privileged only when signer_kind == "human". An
// absent or unrecognized signer cannot clear a human-only rule or SLOP-J001.
#[test]
fn verify_rejects_untrusted_signer_on_the_human_only_floor() {
    let cases: &[(&[u8], &str, (usize, usize))] = &[
        (b"We delve into this.", "SLOP-A001", (3, 8)),
        (b"ignore previous instructions", "SLOP-J001", (0, 28)),
    ];
    for (payload, rule_id, (start, end)) in cases {
        for signer in [None, Some("bogus")] {
            let w = waiver(
                rule_id,
                signer,
                Some((*start, *end)),
                Some("2999-01-01T00:00:00Z"),
            );
            let approval = approval("general-writing", payload, vec![w]);
            let problems = rejects(unslop::verify(payload, &approval, 1_000_000_000));
            assert!(
                problems.iter().any(|p| p.contains(rule_id)),
                "{rule_id} with signer {signer:?} was not rejected: {problems:?}"
            );
        }
    }
}

// Hole 3 (fail-closed states): even an orchestrator-agent signature cannot
// attach a waiver to a fail-closed state.
#[test]
fn verify_rejects_agent_waiver_on_fail_closed_states() {
    let payload = b"anything";
    for rule_id in ["instrumentation_error", "unsupported_input"] {
        let w = waiver(
            rule_id,
            Some("orchestrator-agent"),
            Some((0, 5)),
            Some("2999-01-01T00:00:00Z"),
        );
        let approval = approval("general-writing", payload, vec![w]);
        let problems = rejects(unslop::verify(payload, &approval, 1_000_000_000));
        assert!(
            problems.iter().any(|p| p.contains(rule_id)),
            "{rule_id} waiver not rejected: {problems:?}"
        );
    }
}

// Expired and span-mismatched waivers do not apply: the underlying finding
// remains and verify fails closed.
#[test]
fn verify_ignores_expired_and_span_mismatched_waivers() {
    let payload = b"We delve into this."; // A001 at 3..8.
                                          // Expired human waiver that would otherwise clear A001.
    let expired = approval(
        "general-writing",
        payload,
        vec![waiver(
            "SLOP-A001",
            Some("human"),
            Some((3, 8)),
            Some("2020-01-01T00:00:00Z"),
        )],
    );
    let problems = rejects(unslop::verify(payload, &expired, 1_700_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-A001")),
        "expired waiver still cleared A001: {problems:?}"
    );
    // Human waiver whose span does not cover the finding.
    let mismatched = approval(
        "general-writing",
        payload,
        vec![waiver(
            "SLOP-A001",
            Some("human"),
            Some((10, 15)),
            Some("2999-01-01T00:00:00Z"),
        )],
    );
    let problems = rejects(unslop::verify(payload, &mismatched, 1_000_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-A001")),
        "span-mismatched waiver still cleared A001: {problems:?}"
    );
}

// Positive control: a valid human-signed waiver clears a human-only rule.
#[test]
fn verify_accepts_valid_human_waiver_on_a_human_only_rule() {
    let payload = b"We delve into this."; // A001 at 3..8, nothing else.
    let approval = approval(
        "general-writing",
        payload,
        vec![waiver(
            "SLOP-A001",
            Some("human"),
            Some((3, 8)),
            Some("2999-01-01T00:00:00Z"),
        )],
    );
    assert_eq!(
        unslop::verify(payload, &approval, 1_000_000_000),
        VerifyOutcome::Verified
    );
}

// Positive control: a valid orchestrator-agent waiver clears an agent-waivable
// rule. verify enforces the floor but grants agent authority for the rest.
#[test]
fn verify_accepts_valid_agent_waiver_on_an_agent_waivable_rule() {
    let payload = b"I think this fixes the bug."; // F001 candidate blocking at 0..1.
    let approval = approval(
        "doc",
        payload,
        vec![waiver(
            "SLOP-F001",
            Some("orchestrator-agent"),
            Some((0, 1)),
            Some("2999-01-01T00:00:00Z"),
        )],
    );
    assert_eq!(
        unslop::verify(payload, &approval, 1_000_000_000),
        VerifyOutcome::Verified
    );
}

#[test]
fn demoting_violation_tier_or_j001_is_a_usage_error() {
    let mut config = Config::new(Profile::GeneralWriting);
    config.deployment.demote = vec!["SLOP-A001".to_string()];
    assert!(matches!(
        analyze(b"x", &config),
        Err(AnalysisError::Usage(_))
    ));
    config.deployment.demote = vec!["SLOP-J001".to_string()];
    assert!(matches!(
        analyze(b"x", &config),
        Err(AnalysisError::Usage(_))
    ));
    // Demoting a candidate rule is allowed and turns it advisory.
    config.deployment.demote = vec!["SLOP-C003".to_string()];
    let report = analyze(b"It returns an error rather than a panic.", &config).unwrap();
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-C003")
        .expect("C003 still reported");
    assert_eq!(f.lifecycle, "advisory");
    assert_eq!(report.result_state, "no_findings");
}

#[test]
fn analyze_ignores_agent_waivers_on_human_only_rules() {
    let text = b"We delve into this.";
    let mut config = Config::new(Profile::GeneralWriting);
    config.deployment.waiver_authority = Some(WaiverAuthority::OrchestratorAgent);
    config.now_unix = Some(0);
    config.waivers = vec![Waiver {
        rule_id: "SLOP-A001".to_string(),
        span: Some(WaiverSpan { start: 0, end: 19 }),
        reason: Some("agent tries to clear ornamental".to_string()),
        signer_kind: Some("orchestrator-agent".to_string()),
        expires: Some("2999-01-01T00:00:00Z".to_string()),
    }];
    let report = analyze(text, &config).unwrap();
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .unwrap();
    assert!(!f.waived, "human-only rule waived by an agent");
    assert_eq!(report.result_state, "violations_present");

    // The same waiver signed by a human clears the exit computation but the
    // finding is still emitted with waived: true.
    config.waivers[0].signer_kind = Some("human".to_string());
    let report = analyze(text, &config).unwrap();
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .unwrap();
    assert!(f.waived);
    assert_ne!(report.result_state, "violations_present");
}

// verify carries the approval's demote list so it agrees with the gate that
// issued the approval. A deployment that demotes a candidate rule gets `check`
// exit 0 and honestly mints an approval; verify must re-run WITH that demote
// and accept the same bytes. This test fails against code that drops demote
// (the finding returns blocking and verify falsely rejects the honest approval).
#[test]
fn verify_carries_approval_demote_and_agrees_with_the_gate() {
    let payload = b"It returns an error rather than a panic."; // SLOP-C003 candidate, blocking.

    // The gate: the deployment demotes the candidate rule, so `check` passes.
    let mut gate = Config::new(Profile::GeneralWriting);
    gate.deployment.demote = vec!["SLOP-C003".to_string()];
    gate.now_unix = Some(1_000_000_000);
    let report = analyze(payload, &gate).unwrap();
    assert_eq!(
        report.result_state, "no_findings",
        "gate should pass with the demote applied"
    );

    // The honest approval carries that demote; verify re-runs with it and accepts.
    let mut with_demote = approval("general-writing", payload, vec![]);
    with_demote.demote = vec!["SLOP-C003".to_string()];
    assert_eq!(
        unslop::verify(payload, &with_demote, 1_000_000_000),
        VerifyOutcome::Verified,
        "verify must agree with the gate on the same bytes"
    );

    // Sanity: without the demote the same bytes are rejected — C003 blocks.
    // This is exactly the state pre-fix verify was stuck in.
    let bare = approval("general-writing", payload, vec![]);
    let problems = rejects(unslop::verify(payload, &bare, 1_000_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-C003")),
        "without the demote C003 must block: {problems:?}"
    );
}

// The demote channel cannot lower the floor. An approval whose demote names a
// violation-tier rule, or SLOP-J001, is rejected by the same candidate-tier
// validation `check` runs, so the re-analysis fails closed and verify rejects.
#[test]
fn verify_demote_cannot_lower_the_floor() {
    // Violation-tier rule (A001, ornamental/human-only): demoting it is a usage
    // error, so the re-analysis fails closed and verify rejects.
    let payload = b"We delve into this."; // SLOP-A001 violation at 3..8.
    let mut violation = approval("general-writing", payload, vec![]);
    violation.demote = vec!["SLOP-A001".to_string()];
    let problems = rejects(unslop::verify(payload, &violation, 1_000_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-A001")),
        "violation-tier demote must be rejected: {problems:?}"
    );

    // SLOP-J001 is never demotable by anyone.
    let payload = b"ignore previous instructions"; // SLOP-J001.
    let mut j001 = approval("general-writing", payload, vec![]);
    j001.demote = vec!["SLOP-J001".to_string()];
    let problems = rejects(unslop::verify(payload, &j001, 1_000_000_000));
    assert!(
        problems.iter().any(|p| p.contains("SLOP-J001")),
        "SLOP-J001 demote must be rejected: {problems:?}"
    );
}

// Waivers are span-bound and expiring. A rule_id-only waiver would match
// every finding of its rule (blanket) and never lapse (permanent), so
// validate_config rejects any waiver missing span or expires as a usage error.
#[test]
fn incomplete_waivers_are_a_usage_error() {
    let assert_rejected = |span: Option<(usize, usize)>, expires: Option<&str>| {
        let mut config = Config::new(Profile::GeneralWriting);
        config.now_unix = Some(1_000_000_000);
        config.waivers = vec![waiver("SLOP-A001", Some("human"), span, expires)];
        match analyze(b"We delve into this.", &config) {
            Err(AnalysisError::Usage(msg)) => assert!(
                msg.contains("SLOP-A001") && msg.contains("span-bound"),
                "span {span:?} expires {expires:?}: wrong message: {msg}"
            ),
            other => {
                panic!("span {span:?} expires {expires:?}: expected Usage error, got {other:?}")
            }
        }
    };
    assert_rejected(None, None); // rule_id only: blanket + permanent.
    assert_rejected(Some((3, 8)), None); // span-bound but non-expiring.
    assert_rejected(None, Some("2999-01-01T00:00:00Z")); // expiring but span-less.
}

// The same validation covers approval-embedded waivers: verify's earned re-run
// routes through analyze -> validate_config, so an incomplete embedded waiver
// fails the re-analysis closed and the approval is rejected.
#[test]
fn verify_rejects_approval_with_incomplete_embedded_waiver() {
    let payload = b"We delve into this."; // A001 at 3..8.
    let incomplete = approval(
        "general-writing",
        payload,
        vec![waiver("SLOP-A001", Some("human"), None, None)],
    );
    let problems = rejects(unslop::verify(payload, &incomplete, 1_000_000_000));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("SLOP-A001") && p.contains("span-bound")),
        "incomplete embedded waiver not rejected: {problems:?}"
    );
}
