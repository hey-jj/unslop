//! mechanical-style structural rule: SLOP-M005 structural anomaly.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::{Doc, RegionKind};
use crate::input::Prepared;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-M005"];

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let Some(idx) = super::active(cp, config, "SLOP-M005") else {
        return;
    };
    if let Some(range) = &doc.fence_unclosed {
        hits.push(Hit::new(idx, range.clone()));
    }
    if let Some(range) = &doc.html_unclosed_comment {
        hits.push(Hit::new(idx, range.clone()));
    }
    if let Some(range) = &doc.html_unclosed_script {
        hits.push(Hit::new(idx, range.clone()));
    }
    if let Some(range) = &doc.html_unparseable {
        hits.push(Hit::new(idx, range.clone()));
    }
    if let Some(range) = &doc.numeric_ref_anomaly {
        hits.push(Hit::new(idx, range.clone()));
    }
    let total = prepared.text.len();
    let rule = &cp.pkg.rules[idx];
    let pct = super::param_i64(rule, "raw_html_dominance_pct").unwrap_or(20) as usize;
    // Dominance needs BOTH the ratio and an absolute net-markup floor: the
    // ratio alone tripped on an idiomatic badge-header README (~700 bytes of
    // centered-div/badge markup in a ~1.5 KB file), while every hidden-HTML
    // evasion class is separately fail-closed by the anomalies above and
    // SLOP-Y001, so a small doc no longer needs the ratio as its only guard.
    let floor = super::param_i64(rule, "raw_html_dominance_floor_bytes").unwrap_or(800) as usize;
    if total > 0 && doc.html_bytes >= floor && doc.html_bytes * 100 > pct * total {
        let span = doc
            .regions
            .iter()
            .find(|r| r.kind == RegionKind::Html)
            .map(|r| r.range.clone())
            .unwrap_or(0..0);
        hits.push(Hit::new(idx, span));
    }
}
