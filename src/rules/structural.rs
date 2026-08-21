//! structural fingerprint family: SLOP-X001 boilerplate skeleton,
//! SLOP-X003 oversized body, SLOP-X004 over-structured short doc.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::Prepared;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-X001", "SLOP-X003", "SLOP-X004"];

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    if let Some(idx) = super::active(cp, config, "SLOP-X001") {
        let rule = &cp.pkg.rules[idx];
        let set: Vec<String> = rule
            .params
            .as_table()
            .and_then(|t| t.get("heading_set"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let min = super::param_i64(rule, "min_matches").unwrap_or(3) as usize;
        let matching: Vec<&crate::extract::Heading> = doc
            .headings
            .iter()
            .filter(|h| set.iter().any(|s| s == &h.text.to_ascii_lowercase()))
            .collect();
        if matching.len() >= min {
            hits.push(Hit::new(idx, matching[0].range.clone()));
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-X003") {
        let rule = &cp.pkg.rules[idx];
        let cap = rule
            .params
            .as_table()
            .and_then(|t| t.get("max_words"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get(config.profile.as_str()))
            .and_then(|v| v.as_integer());
        if let Some(cap) = cap {
            if doc.stats.word_count > cap as u64 {
                hits.push(Hit::new(idx, crate::first_char_span(&prepared.text)));
            }
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-X004") {
        let rule = &cp.pkg.rules[idx];
        let max_words = super::param_i64(rule, "max_words").unwrap_or(300) as u64;
        let min_headings = super::param_i64(rule, "min_headings").unwrap_or(4) as usize;
        if doc.stats.word_count <= max_words
            && doc.stats.word_count > 0
            && doc.headings.len() >= min_headings
        {
            hits.push(Hit::new(idx, doc.headings[0].range.clone()));
        }
    }
}
