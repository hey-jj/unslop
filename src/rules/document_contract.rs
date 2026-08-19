//! document-contract family: SLOP-K005, the License section the doc profile
//! requires when configuration supplies the expected wording.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::Prepared;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-K005"];

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let src = prepared.text.as_str();

    let Some(idx) = super::active(cp, config, "SLOP-K005") else {
        return;
    };
    let license_heading = doc
        .headings
        .iter()
        .enumerate()
        .find(|(_, h)| h.text.eq_ignore_ascii_case("license"));
    match (license_heading, &config.deployment.expected_license_wording) {
        // Nothing to check against and no heading: most documents carry no
        // License section, so silence is the honest report.
        (None, None) => {}
        (None, Some(_)) => hits.push(Hit::new(idx, 0..0)),
        (Some((_, h)), None) => {
            let mut hint = Hit::new(idx, h.range.clone());
            hint.force_hint = true;
            hits.push(hint);
        }
        (Some((i, h)), Some(expected)) => {
            let section = doc
                .sections
                .get(i)
                .map(|s| s.range.clone())
                .unwrap_or(h.range.start..src.len());
            let body = collapse_ws(&src[section]);
            if !body.contains(&collapse_ws(expected)) {
                hits.push(Hit::new(idx, h.range.clone()));
            }
        }
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
