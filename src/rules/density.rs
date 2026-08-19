//! density family: integer running sums, population variance via
//! `n*sum_sq - sum^2`, integer cross-multiplication against rational
//! thresholds. Density rules escalate to whole-document review and never
//! substitute for individual hits.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::Prepared;
use crate::Config;

pub const HANDLED: &[&str] = &[
    "SLOP-D001",
    "SLOP-D002",
    "SLOP-D003",
    "SLOP-D004",
    "SLOP-C009",
    "SLOP-L002",
];

fn count_rule_hits(cp: &CompiledPolicy, hits: &[Hit], ids: &[String]) -> u64 {
    hits.iter()
        .filter(|h| ids.iter().any(|id| &cp.pkg.rules[h.rule].id == id))
        .count() as u64
}

fn param_rule_ids(rule: &crate::policy::Rule) -> Vec<String> {
    rule.params
        .as_table()
        .and_then(|t| t.get("count_rules"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let whole = 0..prepared.text.len().min(1);

    for id in ["SLOP-D001", "SLOP-D004"] {
        let Some(idx) = super::active(cp, config, id) else {
            continue;
        };
        let rule = &cp.pkg.rules[idx];
        let ids = param_rule_ids(rule);
        let threshold = super::param_i64(rule, "threshold").unwrap_or(3) as u64;
        let count = count_rule_hits(cp, hits, &ids);
        let per_document = rule
            .params
            .as_table()
            .and_then(|t| t.get("per_document"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let fires = if per_document {
            count >= threshold
        } else {
            let per_words = super::param_i64(rule, "per_words").unwrap_or(500) as u64;
            let words = doc.stats.word_count;
            // Rate check by cross-multiplication, plus an absolute floor so
            // one hit in a short note does not escalate.
            words > 0 && count >= threshold && count * per_words >= threshold * words
        };
        if fires {
            hits.push(Hit::new(idx, whole.clone()));
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-D002") {
        let rule = &cp.pkg.rules[idx];
        let min_bullets = super::param_i64(rule, "min_bullets").unwrap_or(8) as u64;
        let min_pct = super::param_i64(rule, "min_link_bullet_pct").unwrap_or(60) as u64;
        if doc.stats.bullets >= min_bullets
            && doc.stats.bullets_with_link * 100 >= min_pct * doc.stats.bullets
        {
            hits.push(Hit::new(idx, whole.clone()));
        }
    }

    // Rate instruments, not gates: SLOP-C009 over the contrast family and
    // SLOP-L002 over the agentive passive. No threshold param by design, the
    // absence being the deferred per-profile probe. Zero hits emit nothing,
    // since an all-zero line is noise.
    for (id, unit) in [
        ("SLOP-C009", "contrast hits"),
        ("SLOP-L002", "passive hits"),
    ] {
        let Some(idx) = super::active(cp, config, id) else {
            continue;
        };
        let rule = &cp.pkg.rules[idx];
        let ids = param_rule_ids(rule);
        let per_words = super::param_i64(rule, "per_words").unwrap_or(1000) as u64;
        let count = count_rule_hits(cp, hits, &ids);
        let words = doc.stats.word_count;
        if count > 0 && words > 0 && per_words > 0 {
            // One decimal place via integer math, no floats.
            let tenths = count * per_words * 10 / words;
            let mut hit = Hit::new(idx, whole.clone());
            hit.detail = Some(format!(
                "{}.{} per {} words ({} {} / {} words)",
                tenths / 10,
                tenths % 10,
                per_words,
                count,
                unit,
                words
            ));
            hits.push(hit);
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-D003") {
        let rule = &cp.pkg.rules[idx];
        let min_paragraphs = super::param_i64(rule, "min_paragraphs").unwrap_or(5) as usize;
        let cv_pct = super::param_i64(rule, "max_length_cv_pct").unwrap_or(20) as u128;
        let words: Vec<u64> = doc
            .stats
            .paragraph_words
            .iter()
            .copied()
            .filter(|&w| w > 0)
            .collect();
        if words.len() >= min_paragraphs {
            let n = words.len() as u128;
            let s: u128 = words.iter().map(|&w| w as u128).sum();
            let sq: u128 = words.iter().map(|&w| (w as u128) * (w as u128)).sum();
            // cv <= t/100  <=>  100^2 * (n*sq - s^2) <= t^2 * s^2
            if s > 0 && 10_000 * (n * sq - s * s) <= cv_pct * cv_pct * s * s {
                hits.push(Hit::new(idx, whole.clone()));
            }
        }
    }
}
