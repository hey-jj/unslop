//! Section 12.5 and 12.8: property tests and determinism. `analyze` never
//! panics on arbitrary bytes, output is byte-identical across runs, offsets
//! stay in bounds on char boundaries.

mod common;

use proptest::prelude::*;
use unslop::{analyze, Config, Profile};

#[test]
fn determinism_same_input_twice_is_byte_identical() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")).unwrap();
    let config = Config::new(Profile::Doc);
    let a = serde_json::to_string(&analyze(text.as_bytes(), &config).unwrap()).unwrap();
    let b = serde_json::to_string(&analyze(text.as_bytes(), &config).unwrap()).unwrap();
    assert_eq!(a, b);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn analyze_never_panics_on_arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        for profile in [Profile::Doc, Profile::Email, Profile::Doc] {
            let mut config = Config::new(profile);
            config.input_format = profile.default_format();
            let _ = analyze(&bytes, &config);
        }
    }

    #[test]
    fn valid_utf8_reports_hold_the_span_invariant(text in "\\PC{0,400}") {
        let config = Config::new(Profile::Doc);
        if let Ok(report) = analyze(text.as_bytes(), &config) {
            let stripped = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
            common::assert_invariants(stripped, &report);
        }
    }

    // Trigger fidelity must never FALSE-TRIP a real finding: `analyze` may
    // fail for other instrumentation reasons on adversarial input, but it must
    // never emit the trigger-fidelity error, which fires only when a finding's
    // reported span does not render back to the trigger the pattern matched.
    #[test]
    fn r7_never_false_trips_a_real_finding(text in "\\PC{0,400}") {
        for profile in [Profile::Doc, Profile::Doc, Profile::GeneralWriting] {
            let config = Config::new(profile);
            if let Err(e) = analyze(text.as_bytes(), &config) {
                prop_assert!(
                    !format!("{e}").contains("trigger-fidelity"),
                    "trigger fidelity false-tripped on {:?}: {e}", text
                );
            }
        }
    }

    #[test]
    fn analysis_is_deterministic_on_random_text(text in "\\PC{0,300}") {
        let config = Config::new(Profile::GeneralWriting);
        let a = analyze(text.as_bytes(), &config).map(|r| serde_json::to_string(&r).unwrap());
        let b = analyze(text.as_bytes(), &config).map(|r| serde_json::to_string(&r).unwrap());
        match (a, b) {
            (Ok(x), Ok(y)) => prop_assert_eq!(x, y),
            (Err(_), Err(_)) => {}
            _ => prop_assert!(false, "one run failed, one succeeded"),
        }
    }
}
