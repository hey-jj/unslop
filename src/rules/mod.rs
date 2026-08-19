//! Rule evaluation beyond the word-set and regex-set engines: structural,
//! ratio, and document-contract rules. One module per rule family that needs
//! code beyond the shared engines.

pub mod attribution;
pub mod contrast;
pub mod coverage;
pub mod density;
pub mod document_contract;
pub mod duplication;
pub mod emphasis;
pub mod mechanical;
pub mod rendered;
pub mod sentence;
pub mod structural;

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::Prepared;
use crate::views::NormView;
use crate::{Config, Stance};

/// Rule IDs served entirely by the shared word-set and regex-set engines.
pub const ENGINE_RULES: &[&str] = &[
    "SLOP-A001",
    "SLOP-A002",
    "SLOP-A003",
    "SLOP-A004",
    "SLOP-A005",
    "SLOP-A006",
    "SLOP-A007",
    "SLOP-A009",
    "SLOP-A010",
    "SLOP-P001",
    "SLOP-P002",
    "SLOP-P003",
    "SLOP-P004",
    "SLOP-P005",
    "SLOP-M001",
    "SLOP-M002",
    "SLOP-M003",
    "SLOP-M004",
    "SLOP-M006",
    "SLOP-M008",
    "SLOP-S001",
    "SLOP-S003",
    "SLOP-S004",
    "SLOP-S005",
    "SLOP-V001",
    "SLOP-V002",
    "SLOP-V003",
    "SLOP-V004",
    "SLOP-V005",
    "SLOP-V006",
    "SLOP-T001",
    "SLOP-T002",
    "SLOP-T003",
    "SLOP-I001",
    "SLOP-I002",
    "SLOP-I003",
    "SLOP-I004",
    "SLOP-I005",
    "SLOP-I006",
    "SLOP-C001",
    "SLOP-C002",
    "SLOP-C003",
    "SLOP-C004",
    "SLOP-C005",
    "SLOP-C006",
    "SLOP-C008",
    "SLOP-Q001",
    "SLOP-E002",
    "SLOP-R001",
    "SLOP-R002",
    "SLOP-F001",
    "SLOP-F002",
    "SLOP-F003",
    "SLOP-F004",
    "SLOP-O001",
    "SLOP-O002",
    "SLOP-O003",
    "SLOP-O004",
    "SLOP-O006",
    "SLOP-W002",
    "SLOP-J001",
];

/// Every `(rule id, param key)` the implementation actually reads — or whose
/// behavior it implements with the policy value hardcoded (noted inline).
/// The policy-CI param-coverage gate fails when policy.toml declares a param
/// absent from this list and not explicitly disclosed: a declared-but-dead
/// param is exactly how the H003 unusual-scripts silent false negative
/// once shipped, because the older implemented-symbol check was rule-level
/// only.
pub fn implemented_param_keys() -> &'static [(&'static str, &'static str)] {
    &[
        ("SLOP-M005", "unclosed_fence"),
        ("SLOP-M005", "raw_html_dominance_pct"),
        ("SLOP-M005", "raw_html_dominance_floor_bytes"),
        ("SLOP-E001", "emphasized_words"),
        ("SLOP-E001", "followed_within"),
        ("SLOP-E001", "followed_by"),
        ("SLOP-C007", "tail_np_max_bytes"),
        ("SLOP-C007", "clause_window_bytes"),
        ("SLOP-C007", "imperative_openers"),
        ("SLOP-C007", "second_person"),
        ("SLOP-E003", "list_items_with_leading_bold_label"),
        ("SLOP-E004", "closed_class_words"),
        ("SLOP-E005", "min_bold_runs"),
        ("SLOP-M007", "min_words_before_colon"),
        ("SLOP-M007", "max_tail_commas"),
        ("SLOP-C010", "max_endpoint_bytes"),
        ("SLOP-C010", "verb_window_words"),
        ("SLOP-C010", "breadth_signals"),
        ("SLOP-C010", "category_heads"),
        ("SLOP-C010", "motion_verbs"),
        ("SLOP-A008", "of_within_tokens"),
        ("SLOP-L001", "irregular_participles"),
        ("SLOP-L001", "temporal_nouns"),
        ("SLOP-O007", "triggers"),
        ("SLOP-O007", "triggers_after"),
        ("SLOP-O007", "min_capitalized_items"),
        ("SLOP-L002", "count_rules"),
        ("SLOP-L002", "per_words"),
        ("SLOP-L003", "max_words"),
        ("SLOP-L003", "max_clause_commas"),
        ("SLOP-D001", "count_rules"),
        ("SLOP-D001", "threshold"),
        ("SLOP-D001", "per_words"),
        ("SLOP-D002", "min_bullets"),
        ("SLOP-D002", "min_link_bullet_pct"),
        ("SLOP-D003", "min_paragraphs"),
        ("SLOP-D003", "max_length_cv_pct"),
        ("SLOP-D004", "count_rules"),
        ("SLOP-D004", "threshold"),
        ("SLOP-D004", "per_document"),
        ("SLOP-C009", "count_rules"),
        ("SLOP-C009", "per_words"),
        ("SLOP-U001", "shingle_words"),
        ("SLOP-U001", "min_run_words"),
        ("SLOP-U001", "max_reports"),
        ("SLOP-X001", "heading_set"),
        ("SLOP-X001", "min_matches"),
        ("SLOP-X003", "max_words"),
        ("SLOP-X004", "max_words"),
        ("SLOP-X004", "min_headings"),
        ("SLOP-K005", "required_heading"), // hardcoded "license"
        ("SLOP-K005", "expected_wording_from_config"),
        // Y001: the three channels are implemented in render::render_invisible
        // (empty-content skip == min 1 char, dropped_html_text, unused refdefs
        // with a title).
        ("SLOP-Y001", "html_comment_min_text_chars"),
        ("SLOP-Y001", "dropped_raw_html_text"),
        ("SLOP-Y001", "unused_link_definitions_with_prose"),
        // Y002: behavior-descriptors of the (narrow, documented) divergence
        // channel in render::render_divergence.
        ("SLOP-Y002", "compare"),
        ("SLOP-Y002", "ignore"),
        ("SLOP-H001", "emit"), // the section map in every coverage block
        ("SLOP-H002", "emit"), // the excluded-bytes map in every coverage block
        ("SLOP-H002", "flag_excluded_pct"),
        ("SLOP-H003", "mixed_line_endings"),
        ("SLOP-H003", "bom_stripped"),
        // Mixed-script token hint implemented in coverage::evaluate; the
        // evasion itself is closed by the norm-view homoglyph fold (A001).
        ("SLOP-H003", "unusual_scripts_in_identifierlike_prose"),
    ]
}

/// Every rule ID with an implementation symbol. The policy CI test checks
/// this list against the package.
pub fn implemented_rule_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = ENGINE_RULES.to_vec();
    ids.extend(mechanical::HANDLED);
    ids.extend(contrast::HANDLED);
    ids.extend(duplication::HANDLED);
    ids.extend(emphasis::HANDLED);
    ids.extend(density::HANDLED);
    ids.extend(sentence::HANDLED);
    ids.extend(attribution::HANDLED);
    ids.extend(structural::HANDLED);
    ids.extend(document_contract::HANDLED);
    ids.extend(rendered::HANDLED);
    ids.extend(coverage::HANDLED);
    ids
}

pub(crate) fn rule_idx(cp: &CompiledPolicy, id: &str) -> Option<usize> {
    cp.pkg.rules.iter().position(|r| r.id == id)
}

/// True when the rule is active for the profile and not deprecated.
pub(crate) fn active(cp: &CompiledPolicy, config: &Config, id: &str) -> Option<usize> {
    let idx = rule_idx(cp, id)?;
    let rule = &cp.pkg.rules[idx];
    if rule.lifecycle == crate::policy::Lifecycle::Deprecated {
        return None;
    }
    if rule.stance(config.profile) == Stance::Off {
        return None;
    }
    Some(idx)
}

pub(crate) fn param_i64(rule: &crate::policy::Rule, key: &str) -> Option<i64> {
    rule.params.as_table()?.get(key)?.as_integer()
}

pub fn evaluate_structural(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    norm: &NormView,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    mechanical::evaluate(cp, prepared, doc, config, hits);
    contrast::evaluate(cp, prepared, norm, config, hits);
    duplication::evaluate(cp, prepared, norm, config, hits);
    emphasis::evaluate(cp, prepared, doc, config, hits);
    structural::evaluate(cp, prepared, doc, config, hits);
    sentence::evaluate(cp, prepared, doc, norm, config, hits);
    attribution::evaluate(cp, prepared, norm, config, hits);
    document_contract::evaluate(cp, prepared, doc, config, hits);
    rendered::evaluate(cp, prepared, doc, config, hits);
    coverage::evaluate(cp, prepared, doc, config, hits);
    // Density rules run last: they count resolved hits.
    density::evaluate(cp, prepared, doc, config, hits);
}
