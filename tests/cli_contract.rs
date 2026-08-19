//! Section 12.9: CLI contract. Exit codes per state, stdout purity, stderr
//! diagnostics, verify against a mutated payload.

use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_unslop"))
}

fn run_stdin(args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut child = bin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A child asserting a usage error may exit before draining stdin; the
    // resulting EPIPE on this write is expected, not a harness failure.
    let _ = child.stdin.as_mut().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn missing_profile_is_usage_error() {
    let (code, stdout, stderr) = run_stdin(&["check", "-"], b"text");
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "usage errors must not write stdout");
    assert!(stderr.contains("--profile"));
}

#[test]
fn clean_doc_exits_zero_with_pure_json_stdout() {
    let (code, stdout, _) = run_stdin(
        &["check", "--profile", "essay", "-"],
        b"A plain note about the build.\n",
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect("stdout is JSON");
    assert_eq!(v["result_state"], "no_findings");
    assert_eq!(stdout.trim().lines().count(), 1);
}

#[test]
fn violation_exits_10_candidate_exits_20() {
    let (code, stdout, _) = run_stdin(
        &["check", "--profile", "essay", "-"],
        b"We delve into it.\n",
    );
    assert_eq!(code, 10);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["result_state"], "violations_present");

    let (code, stdout, _) = run_stdin(
        &["check", "--profile", "essay", "-"],
        b"It fails rather than recovering.\n",
    );
    assert_eq!(code, 20, "stdout: {stdout}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["result_state"], "candidates_present");
}

#[test]
fn invalid_utf8_exits_40() {
    let (code, stdout, stderr) = run_stdin(
        &["check", "--profile", "essay", "-"],
        &[0x66, 0x6F, 0xFF, 0xFE],
    );
    assert_eq!(code, 40);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["result_state"], "unsupported_input");
    assert!(stderr.contains("unsupported_input"));
}

#[test]
fn over_limit_exits_40() {
    let (code, _, _) = run_stdin(
        &["check", "--profile", "essay", "--max-bytes", "8", "-"],
        b"this is over eight bytes",
    );
    assert_eq!(code, 40);
}

#[test]
fn verify_mismatch_exits_10() {
    let dir = std::env::temp_dir().join("unslop-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let approval_path = dir.join("approval.json");
    // A clean document. verify re-runs the linter, so the approved bytes must
    // actually pass, not merely match the recorded hash.
    let clean = b"Reads a file and returns its bytes.";
    let approval = serde_json::json!({
        "document_sha256": unslop::input::sha256_hex(clean),
        "policy_digest": unslop::policy_digest(),
        "profile": "essay",
        "waivers": [],
    });
    std::fs::write(&approval_path, approval.to_string()).unwrap();

    let (code, stdout, _) = run_stdin(
        &["verify", "--approval", approval_path.to_str().unwrap(), "-"],
        clean,
    );
    assert_eq!(code, 0, "stdout: {stdout}");

    let (code, stdout, stderr) = run_stdin(
        &["verify", "--approval", approval_path.to_str().unwrap(), "-"],
        b"a mutated payload",
    );
    assert_eq!(code, 10);
    assert!(stdout.contains("\"verified\":false"));
    assert!(stderr.contains("hash mismatch"));
}

// A waiver file must carry span and expires on every waiver: a rule_id-only
// entry is a usage error (exit 2), while a complete human-signed waiver still
// suppresses its finding (exit 0, finding retained with waived:true).
#[test]
fn incomplete_waiver_file_is_usage_error_complete_waiver_still_works() {
    let dir = std::env::temp_dir().join("unslop-cli-test-waivers");
    std::fs::create_dir_all(&dir).unwrap();
    let payload = b"We delve into this."; // SLOP-A001 violation at 3..8.

    let incomplete_path = dir.join("incomplete.json");
    std::fs::write(
        &incomplete_path,
        serde_json::json!({ "waivers": [{ "rule_id": "SLOP-A001" }] }).to_string(),
    )
    .unwrap();
    let (code, stdout, stderr) = run_stdin(
        &[
            "check",
            "--profile",
            "essay",
            "--waivers",
            incomplete_path.to_str().unwrap(),
            "-",
        ],
        payload,
    );
    assert_eq!(code, 2, "stdout: {stdout} stderr: {stderr}");
    assert!(stdout.is_empty(), "usage errors must not write stdout");
    assert!(stderr.contains("span-bound"), "stderr: {stderr}");

    let complete_path = dir.join("complete.json");
    std::fs::write(
        &complete_path,
        serde_json::json!({ "waivers": [{
            "rule_id": "SLOP-A001",
            "span": { "start": 3, "end": 8 },
            "reason": "cited verbatim from the upstream title",
            "signer_kind": "human",
            "expires": "2999-01-01T00:00:00Z",
        }] })
        .to_string(),
    )
    .unwrap();
    let (code, stdout, stderr) = run_stdin(
        &[
            "check",
            "--profile",
            "essay",
            "--waivers",
            complete_path.to_str().unwrap(),
            "-",
        ],
        payload,
    );
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    assert!(
        stdout.contains("\"waived\":true"),
        "waived finding must still be reported: {stdout}"
    );
}

#[test]
fn policy_digest_prints_the_embedded_digest() {
    let out = bin().args(["policy", "digest"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        unslop::policy_digest()
    );
}

#[test]
fn an_unknown_format_is_a_usage_error() {
    let (code, _, _) = run_stdin(
        &["check", "--profile", "email", "--format", "commit", "-"],
        b"Hello there.\n",
    );
    assert_eq!(code, 2);
}

#[test]
fn suggest_only_annotates_mechanical_rules() {
    let (_, stdout, _) = run_stdin(
        &["check", "--profile", "essay", "--suggest", "-"],
        b"A dash \xE2\x80\x94 here. We delve too.\n",
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let findings = v["findings"].as_array().unwrap();
    let m001 = findings
        .iter()
        .find(|f| f["rule_id"] == "SLOP-M001")
        .unwrap();
    assert!(m001["suggestion"].is_object());
    let a001 = findings
        .iter()
        .find(|f| f["rule_id"] == "SLOP-A001")
        .unwrap();
    assert!(a001.get("suggestion").is_none() || a001["suggestion"].is_null());
    assert!(v["note"].as_str().unwrap().contains("document hash"));
}

#[test]
fn text_output_is_readable_and_keeps_the_exit_code() {
    let (code, stdout, _) = run_stdin(
        &["check", "--profile", "essay", "--output", "text", "-"],
        b"We delve into the vibrant tapestry, ensuring nothing is left out.\n",
    );
    assert_eq!(code, 10, "violations still exit 10");
    assert!(stdout.starts_with("unslop "), "stdout: {stdout}");
    assert!(stdout.contains("profile essay"));
    assert!(stdout.contains("result violations_present | exit 10"));
    assert!(stdout.contains("SLOP-A001"));
    assert!(stdout.contains("delve"), "the snippet is beside the span");
    assert!(
        stdout.contains("judge: "),
        "judge questions reach the reader"
    );
    assert!(stdout.contains("per 1000 words:"));
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "text output is not JSON"
    );
}

#[test]
fn a_clean_document_in_text_output_exits_zero() {
    let (code, stdout, _) = run_stdin(
        &["check", "--profile", "essay", "--output", "text", "-"],
        b"The rain arrived late on Thursday and stayed for two days.\n",
    );
    assert_eq!(code, 0);
    assert!(stdout.contains("result no_findings | exit 0"), "{stdout}");
    assert!(!stdout.contains("\nfindings\n"), "no findings section");
}

#[test]
fn an_unknown_output_mode_is_a_usage_error() {
    let (code, _, stderr) = run_stdin(
        &["check", "--profile", "essay", "--output", "yaml", "-"],
        b"Plain.\n",
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("json or text"), "stderr: {stderr}");
}

#[test]
fn a_source_path_is_rejected_before_it_is_read() {
    let (code, stdout, stderr) = run_stdin(&["check", "--profile", "doc", "src/lib.rs"], b"");
    assert_eq!(code, 40, "stderr: {stderr}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["result_state"], "unsupported_input");
    assert!(
        v["error"].as_str().unwrap().contains("source path"),
        "{stdout}"
    );
}
