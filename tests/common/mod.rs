#![allow(dead_code)]

use unslop::{analyze, Config, Profile, Report};

pub fn cfg(profile: Profile) -> Config {
    Config::new(profile)
}

pub fn run(text: &str, profile: Profile) -> Report {
    analyze(text.as_bytes(), &cfg(profile)).expect("analyze must succeed")
}

pub fn snippet(f: &unslop::Finding) -> String {
    serde_json::from_str::<String>(f.snippet.get()).expect("snippet is a JSON string")
}

pub fn rule_ids(r: &Report) -> Vec<&str> {
    r.findings.iter().map(|f| f.rule_id.as_str()).collect()
}

pub fn has_rule(r: &Report, id: &str) -> bool {
    r.findings.iter().any(|f| f.rule_id == id)
}

pub fn violations(r: &Report) -> Vec<&unslop::Finding> {
    r.findings
        .iter()
        .filter(|f| f.state == "violation")
        .collect()
}

/// The span invariant plus segmentation coverage checks for one report.
pub fn assert_invariants(source: &str, r: &Report) {
    for f in &r.findings {
        let span = &f.spans[0];
        assert!(
            span.end <= source.len(),
            "{}: span out of bounds",
            f.rule_id
        );
        assert!(span.start <= span.end, "{}: inverted span", f.rule_id);
        assert!(
            source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
            "{}: span not on char boundary",
            f.rule_id
        );
        let slice = &source[span.start..span.end];
        let snip = snippet(f);
        if slice.len() <= 200 {
            assert_eq!(slice, snip, "{}: snippet mismatch", f.rule_id);
        } else {
            assert!(
                slice.as_bytes().starts_with(snip.as_bytes()),
                "{}: capped snippet is not a prefix of the source slice",
                f.rule_id
            );
        }
    }
    // Excluded regions are sorted, disjoint, in bounds, and together with
    // prose bytes cover the whole payload.
    let mut covered = 0usize;
    let mut last_end = 0usize;
    for e in &r.coverage.segmentation.excluded {
        assert!(e.start >= last_end, "excluded regions overlap or unsorted");
        assert!(e.end <= source.len());
        covered += e.end - e.start;
        last_end = e.end;
    }
    assert_eq!(
        covered + r.coverage.segmentation.prose_bytes,
        source.len(),
        "segmentation does not cover the payload"
    );
}
