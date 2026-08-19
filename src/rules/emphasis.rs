//! emphasis family structural rules: SLOP-E001 emphasis-staged contrast,
//! SLOP-E003 bold-label lists, SLOP-E004 title-case headings, and SLOP-E005
//! boldface density. The bold rules use parser emphasis events, never
//! literal asterisks.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::{line_ranges, Prepared};
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-E001", "SLOP-E003", "SLOP-E004", "SLOP-E005"];

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    let src = prepared.text.as_str();
    if let Some(idx) = super::active(cp, config, "SLOP-E001") {
        let rule = &cp.pkg.rules[idx];
        let words: Vec<String> = rule
            .params
            .as_table()
            .and_then(|t| t.get("emphasized_words"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let followers: Vec<String> = rule
            .params
            .as_table()
            .and_then(|t| t.get("followed_by"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let within = super::param_i64(rule, "followed_within").unwrap_or(120) as usize;
        for (range, inner) in &doc.emphasis {
            let word = inner.trim().to_ascii_lowercase();
            if !words.iter().any(|w| w == &word) {
                continue;
            }
            let window_end = crate::widen_to_char_boundaries(
                src,
                range.end..(range.end + within).min(src.len()),
            )
            .end;
            let after = src[range.end..window_end].to_ascii_lowercase();
            let followed = followers.iter().any(|f| {
                let mut at = 0usize;
                while let Some(pos) = after[at..].find(f.as_str()) {
                    let s = at + pos;
                    let before_ok = s == 0
                        || !after[..s]
                            .chars()
                            .next_back()
                            .map(|c| c.is_ascii_alphanumeric())
                            .unwrap_or(false);
                    let e = s + f.len();
                    let after_ok = !after[e..]
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_alphanumeric())
                        .unwrap_or(false);
                    if before_ok && after_ok {
                        return true;
                    }
                    at = s + 1;
                }
                false
            });
            if followed {
                hits.push(Hit::new(idx, range.clone()));
            }
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-E003") {
        let rule = &cp.pkg.rules[idx];
        let min = super::param_i64(rule, "list_items_with_leading_bold_label").unwrap_or(3) as u64;
        if doc.stats.bold_label_items >= min {
            let span = doc.bold_label_ranges.first().cloned().unwrap_or(0..0);
            hits.push(Hit::new(idx, span));
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-E004") {
        let rule = &cp.pkg.rules[idx];
        let closed: Vec<String> = rule
            .params
            .as_table()
            .and_then(|t| t.get("closed_class_words"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();
        for h in &doc.headings {
            let capitalized_closed_class = h
                .text
                .split_whitespace()
                .skip(1)
                .filter_map(|w| {
                    let word = w.trim_matches(|c: char| !c.is_alphanumeric());
                    (!word.is_empty()).then_some(word)
                })
                .any(|word| {
                    word.chars().next().is_some_and(|c| c.is_uppercase())
                        && closed.contains(&word.to_lowercase())
                });
            if capitalized_closed_class {
                hits.push(Hit::new(idx, h.text_range.clone()));
            }
        }
    }

    if let Some(idx) = super::active(cp, config, "SLOP-E005") {
        let min_runs = super::param_i64(&cp.pkg.rules[idx], "min_bold_runs").unwrap_or(3) as usize;
        // Leading bold labels belong to SLOP-E003 and are not counted here,
        // so one shape never reports twice.
        let counted: Vec<&std::ops::Range<usize>> = doc
            .strong_runs
            .iter()
            .filter(|r| !doc.bold_label_ranges.iter().any(|b| b.start == r.start))
            .collect();
        // Blocks split on blank lines, list markers, and headings: the unit
        // is one paragraph or one list item, which is what a reader takes in
        // at once.
        let mut runs_in_block: Vec<&std::ops::Range<usize>> = Vec::new();
        let flush = |runs: &mut Vec<&std::ops::Range<usize>>, hits: &mut Vec<Hit>| {
            if runs.len() >= min_runs {
                let span = runs[0].start..runs[runs.len() - 1].end;
                hits.push(Hit::new(idx, span));
            }
            runs.clear();
        };
        for lr in line_ranges(src) {
            let line = &src[lr.clone()];
            let trimmed = line.trim_start();
            let new_block = trimmed.is_empty()
                || trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || trimmed.starts_with('#')
                || trimmed
                    .split_once(". ")
                    .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            if new_block {
                flush(&mut runs_in_block, hits);
            }
            runs_in_block.extend(
                counted
                    .iter()
                    .copied()
                    .filter(|r| r.start >= lr.start && r.start < lr.end),
            );
        }
        flush(&mut runs_in_block, hits);
    }
}
