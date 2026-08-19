//! Approval records, waivers, the authority floor, and `verify`.

use crate::input::sha256_hex;
use crate::WaiverAuthority;
use serde::{Deserialize, Serialize};

/// The only `signer_kind` string that carries human privilege. Any other
/// value — including an absent one — is untrusted (agent-equivalent). This is
/// a fail-closed reading: a waiver is trusted to clear a human-only rule only
/// when it explicitly names the recognized human signer.
pub const HUMAN_SIGNER: &str = "human";

/// The single authority-floor decision, consulted by both `analyze` (through
/// `report::waiver_decision`) and [`verify`] so the two paths cannot
/// drift. Returns `Ok(())` when a waiver with `signer_kind` may clear
/// `rule_id` under `authority`, or `Err(reason)` when the floor forbids it.
///
/// The floor no configuration and no orchestrator-agent signature can lower:
///
/// * `instrumentation_error` and `unsupported_input` are fail-closed states,
///   never waivable by anyone.
/// * A human-only rule (`human_only == true`, i.e. the ornamental set) and
///   `SLOP-J001` are clearable only by a human-signed waiver.
/// * Every other rule is agent-waivable only when `authority` is
///   `OrchestratorAgent`; otherwise it too needs a human signer.
///
/// The signer is read fail-closed: only the exact [`HUMAN_SIGNER`] string is
/// human-privileged. An absent or unrecognized `signer_kind` is untrusted and
/// gets at most agent privilege, so it can never clear the human-only floor.
pub fn floor_allows(
    signer_kind: Option<&str>,
    rule_id: &str,
    human_only: bool,
    authority: WaiverAuthority,
) -> Result<(), String> {
    if rule_id == "instrumentation_error" || rule_id == "unsupported_input" {
        return Err(format!(
            "waiver on fail-closed state {rule_id} is never valid"
        ));
    }
    if signer_kind == Some(HUMAN_SIGNER) {
        // Human authority clears anything above the fail-closed states.
        return Ok(());
    }
    // Untrusted signer: orchestrator-agent, absent, or an unrecognized string.
    let shown = signer_kind.unwrap_or("<absent>");
    if human_only || rule_id == "SLOP-J001" {
        return Err(format!(
            "waiver on {rule_id} requires a human signer; signer_kind {shown} \
             cannot clear the authority floor"
        ));
    }
    if authority != WaiverAuthority::OrchestratorAgent {
        return Err(format!(
            "agent-signed waiver for {rule_id} rejected: deployment authority is human"
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaiverSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Waiver {
    pub rule_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<WaiverSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Approval {
    pub document_sha256: String,
    pub policy_digest: String,
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver_kind: Option<String>,
    /// The issuing deployment's rule demotions, carried so `verify`'s re-run
    /// applies the same lifecycle softening the gate did. Empty by default so
    /// existing approvals parse and behave exactly as before. The list is
    /// revalidated at verify time through the same candidate-tier-only routine
    /// `check` uses, so it grants the author no authority beyond what waivers
    /// already do and cannot demote a violation-tier rule or `SLOP-J001`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demote: Vec<String>,
    #[serde(default)]
    pub waivers: Vec<Waiver>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyOutcome {
    Verified,
    Mismatch(Vec<String>),
}

/// Parse an RFC 3339 timestamp of the form YYYY-MM-DDTHH:MM:SS with an
/// optional fractional part and a Z or numeric offset, into unix seconds.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        if t.bytes().all(|c| c.is_ascii_digit()) {
            t.parse().ok()
        } else {
            None
        }
    };
    let year = num(0..4)?;
    let month = num(5..7)?;
    let day = num(8..10)?;
    let hour = num(11..13)?;
    let minute = num(14..16)?;
    let second = num(17..19)?;
    if b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b't' && b[10] != b' ') {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut rest = &s[19..];
    if rest.starts_with('.') {
        let frac_end = rest[1..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        rest = &rest[frac_end..];
    }
    let offset_secs: i64 = if rest.is_empty() || rest == "Z" || rest == "z" {
        0
    } else {
        let sign = match rest.as_bytes()[0] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        if rest.len() < 6 || rest.as_bytes()[3] != b':' {
            return None;
        }
        let oh: i64 = rest.get(1..3)?.parse().ok()?;
        let om: i64 = rest.get(4..6)?.parse().ok()?;
        sign * (oh * 3600 + om * 60)
    };
    // Days from civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset_secs)
}

/// Verify a payload against an approval record. An approval must be EARNED,
/// not merely byte-matching: alongside the tuple checks (hash, policy digest,
/// profile, waiver expiry, and the authority floor) `verify` RE-RUNS the
/// analysis under the approval's declared profile with the approval's waiver
/// set, and fails closed if any unwaived blocking finding remains.
///
/// The re-analysis routes through the same [`crate::analyze`] entry point and
/// the same [`floor_allows`] floor the interactive path uses, so a byte-clean
/// approval with empty or under-privileged waivers cannot pass a document
/// that still carries blocking violations. Because the approval record does
/// not carry the issuing deployment's `waiver_authority`, the re-analysis runs
/// with `OrchestratorAgent` authority: agent-waivable rules are the issuer's
/// configured business, but the human-only floor (the ornamental set,
/// `SLOP-J001`, and the two fail-closed states) is enforced regardless.
pub fn verify(input: &[u8], approval: &Approval, now_unix: i64) -> VerifyOutcome {
    let mut problems = Vec::new();
    let got = sha256_hex(input);
    let want = approval
        .document_sha256
        .strip_prefix("sha256:")
        .unwrap_or(&approval.document_sha256);
    if got != want {
        problems.push(format!(
            "document hash mismatch: payload is {got}, approval says {want}"
        ));
    }
    let digest = crate::policy::compute_digest();
    if approval.policy_digest != digest {
        problems.push(format!(
            "policy digest mismatch: embedded policy is {digest}, approval says {}",
            approval.policy_digest
        ));
    }
    let profile = crate::Profile::from_str(&approval.profile);
    if profile.is_none() {
        problems.push(format!("unknown profile {}", approval.profile));
    }
    let human_only = |rule_id: &str| -> bool {
        crate::engine::compiled()
            .ok()
            .and_then(|cp| cp.pkg.rule_by_id(rule_id))
            .map(|r| r.human_only_waiver)
            .unwrap_or(false)
    };
    // Per-waiver validity: expiry (verify fails on any expired waiver,
    // whether or not the underlying finding is present) and the shared floor.
    for w in &approval.waivers {
        if let Some(expires) = &w.expires {
            match parse_rfc3339(expires) {
                Some(t) if t < now_unix => {
                    problems.push(format!("waiver for {} expired at {}", w.rule_id, expires));
                }
                None => {
                    problems.push(format!(
                        "waiver for {} has unreadable expiry {}",
                        w.rule_id, expires
                    ));
                }
                _ => {}
            }
        }
        if let Err(reason) = floor_allows(
            w.signer_kind.as_deref(),
            &w.rule_id,
            human_only(&w.rule_id),
            WaiverAuthority::OrchestratorAgent,
        ) {
            problems.push(reason);
        }
    }
    // Earn the approval: re-run analysis under the approval's profile and
    // waiver set, and fail closed on any unwaived blocking finding.
    if let Some(profile) = profile {
        let mut config = crate::Config::new(profile);
        config.waivers = approval.waivers.clone();
        config.now_unix = Some(now_unix);
        config.deployment.waiver_authority = Some(WaiverAuthority::OrchestratorAgent);
        // Carry the issuing deployment's demotions so verify softens the same
        // candidate findings the gate did. `analyze` runs these through
        // `validate_config` (src/lib.rs), the identical candidate-tier-only
        // check `check` uses: a demote naming a violation-tier rule or
        // `SLOP-J001` fails the re-analysis closed, so the floor cannot be
        // lowered through this channel.
        config.deployment.demote = approval.demote.clone();
        match crate::analyze(input, &config) {
            Ok(report) => {
                let mut blocking: Vec<String> = report
                    .findings
                    .iter()
                    .filter(|f| {
                        !f.waived
                            && f.lifecycle == "blocking"
                            && (f.state == "violation" || f.state == "candidate")
                    })
                    .map(|f| f.rule_id.clone())
                    .collect();
                blocking.sort();
                blocking.dedup();
                if !blocking.is_empty() {
                    problems.push(format!(
                        "unwaived blocking findings remain under profile {}: {}",
                        profile.as_str(),
                        blocking.join(", ")
                    ));
                }
            }
            Err(e) => {
                // A fail-closed re-analysis (instrumentation_error /
                // unsupported_input) can never be approved away.
                problems.push(format!("re-analysis under the approval failed closed: {e}"));
            }
        }
    }
    if problems.is_empty() {
        VerifyOutcome::Verified
    } else {
        VerifyOutcome::Mismatch(problems)
    }
}
