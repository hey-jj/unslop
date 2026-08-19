//! Robustness regressions: assembly stays fast on segment- and hit-dense
//! inputs, closed output pipes never panic, extra positionals and mistyped
//! config values are usage errors, and help lands on stdout.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use unslop::{analyze, AnalysisError, Config, Profile};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_unslop")
}

fn run_stdin(args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut child = Command::new(bin())
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

// ---------------------------------------------------------------------------
// Assembly cost: the per-hit norm-text lookup is a binary search and the
// findings cap stops collection, so a 2 MiB input dense in segments and hits
// completes in seconds, not O(hits x segments) minutes. The bound is generous
// for debug builds and CI noise; the pre-fix behavior was minutes to hours.
// ---------------------------------------------------------------------------

const PERF_BOUND: Duration = Duration::from_secs(30);

fn fill_to(base: &str, unit: &str, bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + unit.len());
    s.push_str(base);
    while s.len() < bytes {
        s.push_str(unit);
    }
    s
}

fn timed_analyze(input: &str) -> (Result<unslop::Report, AnalysisError>, Duration) {
    let config = Config::new(Profile::Essay);
    let start = Instant::now();
    let out = analyze(input.as_bytes(), &config);
    (out, start.elapsed())
}

/// Hit-dense and segment-dense at once: every repetition carries a lexicon
/// violation and a zero-width removal (a segment split).
#[test]
fn zero_width_dense_two_mib_completes_fast() {
    let input = fill_to("", "delve\u{200B} x. ", 2 * 1024 * 1024 - 64);
    let (out, elapsed) = timed_analyze(&input);
    assert!(elapsed < PERF_BOUND, "took {elapsed:?}");
    // Far over the findings cap: fail closed, and fast.
    match out {
        Err(AnalysisError::Instrumentation(m)) => assert!(m.contains("limit"), "{m}"),
        other => panic!("expected the findings cap to fail closed, got {other:?}"),
    }
}

/// Every token is a mixed-script homoglyph fold: the fold pass fragments the
/// segment table while every fold is also a hit.
#[test]
fn confusable_flood_two_mib_completes_fast() {
    let input = fill_to("", "d\u{0435}lve x. ", 2 * 1024 * 1024 - 64);
    let (out, elapsed) = timed_analyze(&input);
    assert!(elapsed < PERF_BOUND, "took {elapsed:?}");
    match out {
        Err(AnalysisError::Instrumentation(m)) => assert!(m.contains("limit"), "{m}"),
        other => panic!("expected the findings cap to fail closed, got {other:?}"),
    }
}

/// Thousands of early matches followed by a long entity-dense tail: each
/// early hit's norm-text lookup must not walk the tail's segments.
#[test]
fn dense_early_match_long_tail_two_mib_completes_fast() {
    let mut input = "We delve. ".repeat(9_000);
    let tail = fill_to("", "a&amp;b ", 2 * 1024 * 1024 - 64 - input.len());
    input.push_str(&tail);
    let (out, elapsed) = timed_analyze(&input);
    assert!(elapsed < PERF_BOUND, "took {elapsed:?}");
    let report = out.expect("under the findings cap");
    assert_eq!(report.result_state, "violations_present");
}

/// One giant no-whitespace single-script confusable token: a single finding
/// at most, so the findings cap never bounds the work. The H003 mixed-script
/// verdict must be decided once per token, not re-walked per confusable char
/// (which was O(token^2): 187s at 256 KiB, hours at 2 MiB).
#[test]
fn tokenless_confusable_run_two_mib_completes_fast() {
    let input = fill_to(
        "",
        "\u{0430}\u{0435}\u{043E}\u{0440}\u{0441}",
        2 * 1024 * 1024 - 64,
    );
    let (out, elapsed) = timed_analyze(&input);
    assert!(elapsed < PERF_BOUND, "took {elapsed:?}");
    // A pure-Cyrillic token never mixes scripts: same verdict as 0.1.2.
    assert_eq!(out.expect("completes").result_state, "no_findings");

    // The mixed-script variant still fires exactly one H003 hint, on the
    // first confusable char of the token.
    let mixed = format!(
        "x{}",
        fill_to("", "\u{0430}\u{0435}\u{043E}\u{0440}\u{0441}", 64 * 1024)
    );
    let (out, elapsed) = timed_analyze(&mixed);
    assert!(elapsed < PERF_BOUND, "took {elapsed:?}");
    let report = out.expect("completes");
    let h003: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-H003")
        .collect();
    assert_eq!(h003.len(), 1, "exactly one first-occurrence hint");
    assert_eq!(h003[0].spans[0].start, 1, "the first confusable char");
}

// ---------------------------------------------------------------------------
// Closed stdout: every output path terminates quietly with its intended exit
// code, never a panic backtrace. `>&-` makes each stdout write fail
// deterministically; the pipe test exercises real EPIPE mid-report.
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn run_with_closed_stdout(args: &[&str], stdin: &[u8]) -> (i32, String) {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(r#""$0" "$@" >&-"#)
        .arg(bin())
        .args(args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[cfg(unix)]
#[test]
fn closed_stdout_never_panics_on_any_output_path() {
    let dir = std::env::temp_dir().join("unslop-closed-stdout");
    std::fs::create_dir_all(&dir).unwrap();
    let approval_path = dir.join("approval.json");
    let clean = b"Reads a file and returns its bytes.";
    std::fs::write(
        &approval_path,
        serde_json::json!({
            "document_sha256": unslop::input::sha256_hex(clean),
            "policy_digest": unslop::policy_digest(),
            "profile": "essay",
            "waivers": [],
        })
        .to_string(),
    )
    .unwrap();
    let approval = approval_path.to_str().unwrap();
    let over_cap = "We delve. ".repeat(10_001);

    let cases: Vec<(Vec<&str>, &[u8], i32)> = vec![
        (vec!["--version"], b"", 0),
        // Success report path.
        (vec!["check", "--profile", "essay", "-"], b"Plain.\n", 0),
        // instrumentation_error JSON path (findings cap).
        (
            vec!["check", "--profile", "essay", "-"],
            over_cap.as_bytes(),
            30,
        ),
        // unsupported_input JSON path.
        (vec!["check", "--profile", "essay", "-"], &[0x66, 0xFF], 40),
        // verify verdicts, both ways.
        (vec!["verify", "--approval", approval, "-"], clean, 0),
        (vec!["verify", "--approval", approval, "-"], b"mutated", 10),
        (vec!["policy", "digest"], b"", 0),
        (vec!["policy", "show"], b"", 0),
        (vec!["policy", "snapshot"], b"", 0),
    ];
    for (args, stdin, want) in cases {
        let (code, stderr) = run_with_closed_stdout(&args, stdin);
        assert!(!stderr.contains("panicked"), "{args:?} panicked: {stderr}");
        assert_eq!(code, want, "{args:?} stderr: {stderr}");
    }
}

/// Real EPIPE: a large report piped into `head` that exits after one byte.
/// The writer keeps its verdict exit code and leaves no backtrace.
#[cfg(unix)]
#[test]
fn report_into_early_closing_pipe_keeps_exit_code() {
    let input = "We delve. ".repeat(5_000);
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(r#""$0" check --profile essay - | head -c 1 >/dev/null; exit "${PIPESTATUS[0]}""#)
        .arg(bin())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "panicked: {stderr}");
    assert_eq!(out.status.code(), Some(10), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// Positional strictness and config typing.
// ---------------------------------------------------------------------------

#[test]
fn second_positional_is_usage_error() {
    let (code, stdout, stderr) = run_stdin(&["check", "--profile", "essay", "a.md", "b.md"], b"");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("second path"), "stderr: {stderr}");

    let (code, _, stderr) = run_stdin(&["verify", "--approval", "x.json", "a.md", "b.md"], b"");
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("second path"), "stderr: {stderr}");
}

#[test]
fn wrong_type_config_value_is_usage_error() {
    let dir = std::env::temp_dir().join("unslop-config-types");
    std::fs::create_dir_all(&dir).unwrap();
    let cases = [
        (
            "float.toml",
            "expected_license_wording = 0.1",
            "expected_license_wording",
        ),
        ("authority.toml", "waiver_authority = 3", "waiver_authority"),
        ("demote.toml", "demote = \"SLOP-C902\"", "demote"),
    ];
    for (name, body, key) in cases {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        let (code, stdout, stderr) = run_stdin(
            &[
                "check",
                "--profile",
                "essay",
                "--config",
                p.to_str().unwrap(),
                "-",
            ],
            b"Plain.\n",
        );
        assert_eq!(code, 2, "{name} stderr: {stderr}");
        assert!(stdout.is_empty(), "{name} wrote stdout");
        assert!(stderr.contains(key), "{name} stderr: {stderr}");
    }
    // A correctly typed value still loads.
    let p = dir.join("ok.toml");
    std::fs::write(&p, "expected_license_wording = \"MIT\"").unwrap();
    let (code, _, stderr) = run_stdin(
        &[
            "check",
            "--profile",
            "essay",
            "--config",
            p.to_str().unwrap(),
            "-",
        ],
        b"Plain.\n",
    );
    assert_eq!(code, 0, "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// Help lands on stdout, on the root and on every subcommand.
// ---------------------------------------------------------------------------

#[test]
fn help_prints_usage_to_stdout() {
    for args in [
        &["--help"][..],
        &["-h"][..],
        &["check", "--help"][..],
        &["check", "-h"][..],
        &["verify", "--help"][..],
        &["policy", "--help"][..],
    ] {
        let (code, stdout, stderr) = run_stdin(args, b"");
        assert_eq!(code, 0, "{args:?} stderr: {stderr}");
        assert!(stdout.contains("usage:"), "{args:?} stdout: {stdout}");
        assert!(stderr.is_empty(), "{args:?} stderr: {stderr}");
    }
    // The synopsis documents the alias, the limit, and the exit codes.
    let (_, stdout, _) = run_stdin(&["--help"], b"");
    for needle in [
        "analyze",
        "2 MiB",
        "exit codes",
        "-V",
        "-h",
        "essay",
        "markdown",
    ] {
        assert!(stdout.contains(needle), "usage lacks {needle}: {stdout}");
    }
    // No arguments at all stays a stderr usage error.
    let (code, stdout, stderr) = run_stdin(&[], b"");
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("usage:"));
}
