//! Output schema, deterministic ordering, escaping, and the emit-time span
//! invariant. A mapping bug becomes `instrumentation_error`, never a wrong
//! finding.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::{Doc, RegionKind};
use crate::input::Prepared;
use crate::policy::{Lifecycle, Tier};
use crate::{AnalysisError, Config, Stance, WaiverAuthority};
use serde::Serialize;
use serde_json::value::RawValue;
use std::ops::Range;

pub const SNIPPET_CAP: usize = 200;

#[derive(Serialize, Clone, Debug)]
pub struct SpanOut {
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct Suggestion {
    pub start: usize,
    pub end: usize,
    pub replace_with: String,
}

#[derive(Serialize, Debug)]
pub struct Finding {
    pub rule_id: String,
    pub family: String,
    pub state: String,
    pub lifecycle: String,
    pub waived: bool,
    pub spans: Vec<SpanOut>,
    pub view: String,
    /// Where the span sits in the document, projected from the segmentation
    /// the extractor already computed: heading, fenced-code, blockquote, or
    /// prose. This package treats a blockquote as the quotation container,
    /// so quoted material reports as blockquote rather than as a fifth
    /// label, and inline code shares the code label with fences.
    pub container: String,
    pub snippet: Box<RawValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    pub provenance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Suggestion>,
    pub message: String,
}

#[derive(Serialize, Debug)]
pub struct PolicyRef {
    pub version: String,
    pub digest: String,
}

#[derive(Serialize, Debug)]
pub struct DocumentRef {
    pub sha256: String,
    pub bytes: usize,
    pub encoding: String,
    pub input_format: String,
    pub profile: String,
}

#[derive(Serialize, Debug)]
pub struct ExcludedOut {
    pub start: usize,
    pub end: usize,
    pub reason: String,
}

#[derive(Serialize, Debug)]
pub struct Segmentation {
    pub prose_bytes: usize,
    pub excluded: Vec<ExcludedOut>,
}

#[derive(Serialize, Debug)]
pub struct SectionOut {
    pub title: String,
    pub level: u32,
    pub start: usize,
    pub end: usize,
}

/// Findings per family with the rate that produced them. Presentation over
/// the findings already computed, never an input to the exit code.
#[derive(Serialize, Debug)]
pub struct FamilyRate {
    pub family: String,
    pub findings: usize,
    /// Rate per 1000 words, one decimal, computed in integer arithmetic.
    pub per_1000_words: String,
}

#[derive(Serialize, Debug)]
pub struct DensityStats {
    pub word_count: u64,
    pub byte_len: usize,
    pub families: Vec<FamilyRate>,
}

#[derive(Serialize, Debug)]
pub struct Coverage {
    pub sections: Vec<SectionOut>,
    pub segmentation: Segmentation,
    pub density: DensityStats,
    pub notes: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct Report {
    pub schema_version: &'static str,
    pub tool_version: &'static str,
    pub policy: PolicyRef,
    pub document: DocumentRef,
    pub result_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub findings: Vec<Finding>,
    pub coverage: Coverage,
}

impl Report {
    /// Stable exit code with precedence 30 > 40 > 10 > 20 > 0. The error
    /// states are produced by the binary from `AnalysisError`, so a report
    /// only ever maps to 10, 20, or 0.
    pub fn exit_code(&self) -> i32 {
        match self.result_state.as_str() {
            "instrumentation_error" => 30,
            "unsupported_input" => 40,
            "violations_present" => 10,
            "candidates_present" => 20,
            _ => 0,
        }
    }
}

/// JSON string literal with every C0 and C1 control rendered as `\uXXXX`.
pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 || ((c as u32) >= 0x7F && (c as u32) <= 0x9F) => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Trigger fidelity: does the reported source span still carry the `trigger`
/// the pattern matched? Two independent reconstructions of what the span renders
/// to are checked, and EITHER carrying the trigger passes.
///
/// `raw_slice` is the raw source bytes re-rendered through `render_key`
/// (entity/escape decode, zero-width and HTML-markup removal, NFC, whitespace-
/// run folding, ASCII case-folding); it covers non-norm hits (raw/code/link/
/// heading scans) whose span never went through a mapping. `norm_text` is the
/// norm text the source span overlaps, assembled by `source_span_norm_text` with
/// every exclusion (inline and fenced code, HTML markup, link URLs, autolinks)
/// and mapping (entities, escapes, softbreaks, owned content) applied — Identity
/// segments CLIPPED to their actual source-overlap sub-slice, Mapped segments
/// contributing their whole norm text. It is computed for every hit (norm or
/// not); the clip is what stops a span displaced onto one byte of a
/// trigger-bearing paragraph from inheriting the paragraph's text and passing.
///
/// The test is CONTAINMENT, not equality: a Mapped segment expands to its whole
/// source range, so a legitimate span surrounds the trigger. A span no
/// reconstruction carries is a mapping bug. An empty trigger key (all
/// markup/whitespace) is unverifiable and accepted.
///
/// `decoded` is the third reconstruction, for hits found in a
/// DECODED link destination: the parser-decoded text of the region the span
/// maps into, carried on the hit. It is checked by direct containment — the
/// exact pulldown decode, no entity-table dependence — because the raw
/// spelling may hide the trigger behind references OUTSIDE the crate's
/// enumerated table (`&lowbar;`, `&period;`), which `render_key` cannot
/// resolve; routing those spans through the render_key bridge aborted the
/// whole report (exit 30) on exactly the entity-hidden tracking URLs the
/// decoded scan exists to catch. Like the norm-text reconstruction, the
/// engine binds `decoded` to the hit's own region, so it vouches only for
/// the span it maps to. Empty when the hit is not from the decoded pass.
fn slice_carries_trigger(raw_slice: &str, norm_text: &str, decoded: &str, trigger: &str) -> bool {
    if !decoded.is_empty() && decoded.contains(trigger) {
        return true;
    }
    let want = crate::views::render_key(trigger);
    if want.is_empty() {
        return true;
    }
    crate::views::render_key(raw_slice).contains(&want)
        || crate::views::render_key(norm_text).contains(&want)
}

fn snippet_raw(src: &str, span: &Range<usize>) -> Result<Box<RawValue>, AnalysisError> {
    let full = src.get(span.clone()).ok_or_else(|| {
        AnalysisError::Instrumentation(format!(
            "span {}..{} does not index the payload on a char boundary",
            span.start, span.end
        ))
    })?;
    // The span invariant: the emitted bytes are exactly the source slice.
    if full.as_bytes() != &src.as_bytes()[span.start..span.end] {
        return Err(AnalysisError::Instrumentation(format!(
            "span invariant failed at {}..{}",
            span.start, span.end
        )));
    }
    let mut cap = full.len().min(SNIPPET_CAP);
    while cap > 0 && !full.is_char_boundary(cap) {
        cap -= 1;
    }
    let escaped = escape_json_string(&full[..cap]);
    RawValue::from_string(escaped)
        .map_err(|e| AnalysisError::Instrumentation(format!("snippet escape: {e}")))
}

/// The container label for a span, read off the extractor's own regions and
/// flags. No second segmentation mechanism: heading and blockquote come from
/// the norm-view flags, code from the recorded code regions.
fn container_for(doc: &Doc, norm: &crate::views::NormView, span: &Range<usize>) -> &'static str {
    if doc
        .headings
        .iter()
        .any(|h| h.range.start <= span.start && span.start < h.range.end)
    {
        return "heading";
    }
    // Full containment for code: a prose sentence that opens with an inline
    // code span is prose, and only a span that lives entirely inside code is
    // labelled code.
    if doc
        .code_regions
        .iter()
        .any(|c| c.range.start <= span.start && span.end <= c.range.end)
    {
        return "fenced-code";
    }
    for seg in &norm.segs {
        if seg.src.start <= span.start && span.start < seg.src.end {
            if seg.flags & crate::extract::F_QUOTED != 0 {
                return "blockquote";
            }
            if seg.flags & crate::extract::F_HEADING != 0 {
                return "heading";
            }
            break;
        }
    }
    "prose"
}

fn section_for(doc: &Doc, offset: usize) -> Option<String> {
    doc.sections
        .iter()
        .filter(|s| s.range.start <= offset && offset < s.range.end)
        .max_by_key(|s| (s.level, s.range.start))
        .map(|s| s.title.clone())
}

fn suggestion_for(
    rule: &crate::policy::Rule,
    span: &Range<usize>,
    matched: &str,
) -> Option<Suggestion> {
    // A lexicon that carries the plain replacement supplies the fix
    // directly, matching the case of the first letter it replaces.
    if let Some(plain) = rule.swaps.get(&matched.to_lowercase()) {
        let replace_with = match matched.chars().next() {
            Some(c) if c.is_uppercase() => {
                let mut out = plain.to_string();
                let head: String = out.drain(..1).collect();
                head.to_uppercase() + &out
            }
            _ => plain.clone(),
        };
        return Some(Suggestion {
            start: span.start,
            end: span.end,
            replace_with,
        });
    }
    let replace_with = match rule.id.as_str() {
        // The dash patterns that consume a neighbouring letter must put that
        // letter back, so the suggestion is the comma between whatever the
        // span actually covers.
        "SLOP-M001" => {
            let head = matched.chars().next().filter(|c| c.is_alphabetic());
            let tail = matched.chars().next_back().filter(|c| c.is_alphabetic());
            let mut out = String::new();
            if let Some(c) = head {
                out.push(c);
            }
            out.push_str(", ");
            if let Some(c) = tail {
                out.push(c);
            }
            out
        }
        "SLOP-M002" => ".".to_string(),
        "SLOP-M003" | "SLOP-M006" | "SLOP-T001" | "SLOP-I001" => String::new(),
        _ => return None,
    };
    Some(Suggestion {
        start: span.start,
        end: span.end,
        replace_with,
    })
}

struct WaiverDecision {
    waived: bool,
    notes: Vec<String>,
}

fn waiver_decision(
    cp: &CompiledPolicy,
    config: &Config,
    rule_id: &str,
    human_only: bool,
    span: &Range<usize>,
) -> WaiverDecision {
    let mut notes = Vec::new();
    for w in &config.waivers {
        if w.rule_id != rule_id {
            continue;
        }
        if cp.pkg.rule_by_id(&w.rule_id).is_none() {
            notes.push(format!("waiver for unknown rule {} ignored", w.rule_id));
            continue;
        }
        if let Some(ws) = &w.span {
            if !(ws.start <= span.start && ws.end >= span.end) {
                continue;
            }
        }
        if let (Some(expires), Some(now)) = (&w.expires, config.now_unix) {
            match crate::waiver::parse_rfc3339(expires) {
                Some(t) if t < now => {
                    notes.push(format!("expired waiver for {} ignored", w.rule_id));
                    continue;
                }
                None => {
                    notes.push(format!(
                        "waiver for {} has an unreadable expiry and is ignored",
                        w.rule_id
                    ));
                    continue;
                }
                _ => {}
            }
        }
        // The authority floor is the single shared decision in
        // `waiver::floor_allows`; the interactive path and `verify` cannot
        // drift because both consult it. A waiver is human-privileged only
        // when it names the recognized human signer; an absent or
        // unrecognized `signer_kind` is untrusted and gets at most agent
        // privilege.
        let authority = config
            .deployment
            .waiver_authority
            .unwrap_or(WaiverAuthority::Human);
        if let Err(reason) =
            crate::waiver::floor_allows(w.signer_kind.as_deref(), rule_id, human_only, authority)
        {
            notes.push(reason);
            continue;
        }
        return WaiverDecision {
            waived: true,
            notes,
        };
    }
    WaiverDecision {
        waived: false,
        notes,
    }
}

pub fn assemble(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    norm: &crate::views::NormView,
    config: &Config,
    hits: Vec<Hit>,
) -> Result<Report, AnalysisError> {
    let src = prepared.text.as_str();
    let mut notes: Vec<String> = Vec::new();
    if prepared.bom_stripped {
        notes.push("leading BOM stripped; offsets index the post-BOM payload".to_string());
    }
    if prepared.mixed_line_endings {
        notes.push("mixed line endings".to_string());
    }

    let mut findings: Vec<Finding> = Vec::new();
    let mut adversarial = false;

    for hit in &hits {
        let rule = &cp.pkg.rules[hit.rule];
        if rule.lifecycle == Lifecycle::Deprecated {
            continue;
        }
        let stance = rule.stance(config.profile);
        if stance == Stance::Off {
            continue;
        }
        // Quotation suppression: rules in this list drop their quoted hits
        // entirely — a candidate-tier rule has no lower blocking state to
        // downgrade to, and a quoted idiom is the quoted author's diction.
        if hit.quoted && cp.pkg.quotation_suppress.iter().any(|id| id == &rule.id) {
            continue;
        }
        // The findings cap is enforced HERE, before the per-hit work, so an
        // input that floods the detector stops at the limit instead of paying
        // snippet extraction and trigger fidelity for every excess hit. Fail
        // closed exactly as the post-loop check did.
        if findings.len() >= config.limits.max_findings {
            return Err(AnalysisError::Instrumentation(format!(
                "findings exceed the {} limit",
                config.limits.max_findings
            )));
        }
        let mut tier = rule.tier;
        let mut lifecycle = rule.lifecycle;
        let mut provenance = "author".to_string();

        if hit.force_hint {
            tier = Tier::CoverageHint;
            lifecycle = Lifecycle::Advisory;
        } else if hit.force_candidate && tier == Tier::Violation {
            tier = Tier::Candidate;
        }
        if stance == Stance::Relax {
            match tier {
                Tier::Violation => tier = Tier::Candidate,
                Tier::Candidate => lifecycle = Lifecycle::Advisory,
                Tier::CoverageHint => {}
            }
        }
        if hit.quoted
            && tier == Tier::Violation
            && cp.pkg.quotation_downgrade.iter().any(|id| id == &rule.id)
        {
            tier = Tier::Candidate;
            provenance = "claimed-quotation".to_string();
        }
        if config.deployment.demote.iter().any(|id| id == &rule.id) {
            lifecycle = Lifecycle::Advisory;
        }
        if rule.id == "SLOP-J001" {
            adversarial = true;
        }

        let decision = waiver_decision(cp, config, &rule.id, rule.human_only_waiver, &hit.span);
        notes.extend(decision.notes);

        let snippet = snippet_raw(src, &hit.span)?;
        // Trigger fidelity: the reported source slice, re-rendered through
        // the same view transforms, must still carry the trigger the pattern
        // matched. The comparison is whitespace-run-folded and case-folded via
        // `render_key`, and by CONTAINMENT rather than equality — a Mapped
        // segment expands `to_source` to its whole source range, so the slice
        // legitimately surrounds the trigger (softbreaks, entities, escapes,
        // inline-tag fusion, owned content). A slice that does not carry the
        // trigger is a mapping bug: fail closed as instrumentation rather than
        // emit a finding at the wrong bytes.
        if let Some(trigger) = &hit.trigger {
            if !slice_carries_trigger(
                &src[hit.span.clone()],
                &norm.source_span_norm_text(&hit.span),
                hit.decoded.as_deref().unwrap_or(""),
                trigger,
            ) {
                return Err(AnalysisError::Instrumentation(format!(
                    "trigger-fidelity: source slice {}..{} does not render to the matched trigger",
                    hit.span.start, hit.span.end
                )));
            }
        }
        let suggestion = if config.suggest && !hit.force_hint {
            suggestion_for(rule, &hit.span, &src[hit.span.clone()])
        } else {
            None
        };
        findings.push(Finding {
            rule_id: rule.id.clone(),
            family: rule.family.clone(),
            state: tier.as_str().to_string(),
            lifecycle: lifecycle.as_str().to_string(),
            waived: decision.waived,
            spans: vec![SpanOut {
                start: hit.span.start,
                end: hit.span.end,
            }],
            view: rule.view.as_str().to_string(),
            container: container_for(doc, norm, &hit.span).to_string(),
            snippet,
            section: section_for(doc, hit.span.start),
            provenance,
            suggestion,
            message: match &hit.detail {
                Some(d) => format!("{}: {d}", rule.name.replace('-', " ")),
                None => rule.name.replace('-', " "),
            },
        });
    }

    // Deterministic ordering, then merge duplicates of the same rule at the
    // same span.
    findings.sort_by(|a, b| {
        (a.spans[0].start, a.spans[0].end, a.rule_id.as_str()).cmp(&(
            b.spans[0].start,
            b.spans[0].end,
            b.rule_id.as_str(),
        ))
    });
    findings.dedup_by(|a, b| {
        a.rule_id == b.rule_id
            && a.spans[0].start == b.spans[0].start
            && a.spans[0].end == b.spans[0].end
            && a.state == b.state
    });

    if adversarial {
        notes.push(
            "injection pattern present: adversarial mode, all candidates require human adjudication"
                .to_string(),
        );
    }

    let mut has_violation = false;
    let mut has_candidate = false;
    for f in &findings {
        if f.waived || f.lifecycle != "blocking" {
            continue;
        }
        match f.state.as_str() {
            "violation" => has_violation = true,
            "candidate" => has_candidate = true,
            _ => {}
        }
    }
    let result_state = if has_violation {
        "violations_present"
    } else if has_candidate {
        "candidates_present"
    } else {
        "no_findings"
    };

    let prose_bytes: usize = doc
        .regions
        .iter()
        .filter(|r| r.kind == RegionKind::Prose)
        .map(|r| r.range.len())
        .sum();
    let excluded = doc
        .regions
        .iter()
        .filter(|r| r.kind != RegionKind::Prose)
        .map(|r| ExcludedOut {
            start: r.range.start,
            end: r.range.end,
            reason: r.kind.reason().to_string(),
        })
        .collect();
    let sections = doc
        .sections
        .iter()
        .map(|s| SectionOut {
            title: s.title.clone(),
            level: s.level,
            start: s.range.start,
            end: s.range.end,
        })
        .collect();

    // Per-family rates. Waived findings and hints still count: the block
    // describes the document, and the exit code is computed elsewhere from
    // the findings themselves.
    let words = doc.stats.word_count;
    let mut per_family: std::collections::BTreeMap<&str, usize> = Default::default();
    for f in &findings {
        *per_family.entry(f.family.as_str()).or_default() += 1;
    }
    let families = per_family
        .into_iter()
        .map(|(family, count)| {
            let tenths = (count as u64 * 1000 * 10).checked_div(words).unwrap_or(0);
            FamilyRate {
                family: family.to_string(),
                findings: count,
                per_1000_words: format!("{}.{}", tenths / 10, tenths % 10),
            }
        })
        .collect();
    let density = DensityStats {
        word_count: words,
        byte_len: prepared.text.len(),
        families,
    };

    let note = if config.suggest {
        Some(
            "applying any suggestion changes the document hash and invalidates any approval"
                .to_string(),
        )
    } else {
        None
    };

    Ok(Report {
        schema_version: crate::SCHEMA_VERSION,
        tool_version: crate::TOOL_VERSION,
        policy: PolicyRef {
            version: cp.pkg.version.clone(),
            digest: cp.pkg.digest.clone(),
        },
        document: DocumentRef {
            sha256: prepared.sha256.clone(),
            bytes: prepared.original_len,
            encoding: "utf-8".to_string(),
            input_format: config.input_format.as_str().to_string(),
            profile: config.profile.as_str().to_string(),
        },
        result_state: result_state.to_string(),
        note,
        findings,
        coverage: Coverage {
            sections,
            segmentation: Segmentation {
                prose_bytes,
                excluded,
            },
            density,
            notes,
        },
    })
}

/// Human-readable rendering of a report: one block per finding with its
/// span, the verbatim snippet, and the judge question where the rule has
/// one. The JSON stays the machine surface, and this is the surface a person
/// reads. Judge questions come from the compiled package rather than the
/// report, so the schema does not carry text that is already in the policy.
pub fn render_text(report: &Report) -> String {
    use std::fmt::Write;
    let pkg = crate::engine::compiled().ok().map(|cp| &cp.pkg);
    let judge_of = |id: &str| -> Option<String> {
        pkg.and_then(|p| p.rule_by_id(id))
            .and_then(|r| r.judge.clone())
    };
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(
        w,
        "unslop {} | policy {} | profile {}",
        report.tool_version, report.policy.version, report.document.profile
    );
    let _ = writeln!(
        w,
        "document {} | {} bytes | {} words",
        &report.document.sha256[..16.min(report.document.sha256.len())],
        report.document.bytes,
        report.coverage.density.word_count
    );
    let _ = writeln!(
        w,
        "result {} | exit {}",
        report.result_state,
        report.exit_code()
    );

    let (mut gating, mut advisory): (Vec<&Finding>, Vec<&Finding>) = (Vec::new(), Vec::new());
    for f in &report.findings {
        if f.state == "coverage_hint" || f.lifecycle != "blocking" {
            advisory.push(f);
        } else {
            gating.push(f);
        }
    }

    let section = |title: &str, list: &[&Finding], w: &mut String| {
        if list.is_empty() {
            return;
        }
        let _ = writeln!(w, "\n{title}");
        for f in list {
            let span = &f.spans[0];
            let waived = if f.waived { " | waived" } else { "" };
            let _ = writeln!(
                w,
                "  {} | {} | {} | {} | bytes {}..{}{}",
                f.rule_id, f.state, f.family, f.container, span.start, span.end, waived
            );
            let snippet: String = serde_json::from_str(f.snippet.get()).unwrap_or_default();
            let snippet = snippet.replace('\n', " ");
            if !snippet.trim().is_empty() {
                let _ = writeln!(w, "      {}", snippet.trim());
            }
            let _ = writeln!(w, "      {}", f.message);
            if let Some(j) = judge_of(&f.rule_id) {
                let _ = writeln!(w, "      judge: {j}");
            }
            if let Some(s) = &f.suggestion {
                let _ = writeln!(w, "      fix: replace with {:?}", s.replace_with);
            }
        }
    };
    section("findings", &gating, w);
    section("hints", &advisory, w);

    if !report.coverage.density.families.is_empty() {
        let rates: Vec<String> = report
            .coverage
            .density
            .families
            .iter()
            .map(|f| format!("{} {} ({})", f.family, f.per_1000_words, f.findings))
            .collect();
        let _ = writeln!(w, "\nper 1000 words: {}", rates.join(", "));
    }
    let _ = writeln!(
        w,
        "coverage: {} of {} bytes read as prose, {} sections",
        report.coverage.segmentation.prose_bytes,
        report.coverage.density.byte_len,
        report.coverage.sections.len()
    );
    for n in &report.coverage.notes {
        let _ = writeln!(w, "note: {n}");
    }
    out
}

#[cfg(test)]
mod trigger_fidelity_tests {
    use super::slice_carries_trigger;

    // The raw-slice reconstruction alone verifies these structured divergences
    // (empty norm-text arg, as a non-norm hit would supply).
    #[test]
    fn raw_slice_legitimate_divergences_verify() {
        // Softbreak: norm folds "\n" to a space; the source keeps the newline.
        assert!(slice_carries_trigger("not\njust", "", "", "not just"));
        // Enumerated entity: to_source expands the Mapped em-dash to `&mdash;`.
        assert!(slice_carries_trigger("a&mdash;b", "", "", "a\u{2014}b"));
        // Backslash escape resolved in the norm view.
        assert!(slice_carries_trigger(r"foo\*bar", "", "", "foo*bar"));
        // Inline-tag fusion: the norm reads "delve", the slice carries markup.
        assert!(slice_carries_trigger("de<b></b>lve", "", "", "delve"));
        // Element-boundary space kept across inline markup.
        assert!(slice_carries_trigger(
            "game <i>changer</i>",
            "",
            "",
            "game changer"
        ));
        // Case-folding.
        assert!(slice_carries_trigger("DELVE", "", "", "delve"));
        // Whole-source expansion of a Mapped (owned) segment.
        assert!(slice_carries_trigger(
            "A comprehensive tapestry to delve into",
            "",
            "",
            "delve"
        ));
    }

    // An INVERSION of an earlier premise. Without the barrier, the norm view
    // fused "de"+"lve" across the excluded inline-code gap into "delve", and
    // the norm-text reconstruction certified that fusion as a legitimate
    // finding. That fusion WAS the SLOP-A001 false positive — the reader sees
    // "de x lve", never "delve" — so the inline-code barrier interposes U+FFFD
    // in the norm view, and the pipeline supplies "de\u{FFFD}lve" (not
    // "delve") as the span's norm text. A hit assembled across inline code can
    // no longer exist, and were one ever produced, trigger fidelity fails it
    // closed.
    #[test]
    fn norm_text_reconstruction_respects_inline_code_barrier() {
        // The barrier norm text does not carry the trigger: fail closed.
        assert!(!slice_carries_trigger(
            "de`x`lve",
            "de\u{FFFD}lve",
            "",
            "delve"
        ));
        // A gross desync still fails BOTH reconstructions.
        assert!(!slice_carries_trigger("de`x`lve", "parser", "", "delve"));
        // The norm-text path itself still works where the norm LEGITIMATELY
        // diverges from the raw slice: a decoded entity's norm text verifies
        // a trigger the raw bytes don't spell.
        assert!(slice_carries_trigger("x", "a\u{2014}b", "", "a\u{2014}b"));
    }

    // A span that points at the wrong bytes must fail closed on both paths.
    #[test]
    fn corrupted_span_fails_closed() {
        assert!(!slice_carries_trigger("The parser reads", "", "", "delve"));
        // Off-by-one into a neighbouring word.
        assert!(!slice_carries_trigger("elve", "", "", "delve"));
        assert!(!slice_carries_trigger("", "", "", "delve"));
    }

    // A hit from the decoded-destination pass verifies against
    // the PARSER-decoded text — full HTML5 entity semantics — where the
    // render_key bridge (enumerated entity table only) cannot resolve the
    // raw spelling. Routed through render_key instead, this shape would
    // abort the whole report (exit 30).
    #[test]
    fn decoded_destination_reconstruction_verifies_outside_entity_table() {
        // Raw slice spells `&lowbar;`; render_key leaves it literal, so the
        // raw path fails — the decoded text carries the trigger.
        assert!(slice_carries_trigger(
            "https://e/?utm&lowbar;source=chatgpt",
            "",
            "https://e/?utm_source=chatgpt",
            "utm_source=chatgpt"
        ));
        // A desynced decoded text still fails closed.
        assert!(!slice_carries_trigger(
            "https://e/?utm&lowbar;source=chatgpt",
            "",
            "https://e/?clean",
            "utm_source=chatgpt"
        ));
    }
}
