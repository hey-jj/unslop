//! Deterministic detector and coverage instrument for the patterns that mark
//! writing as machine-generated.
//!
//! The library performs no I/O and no clock reads. `analyze` is a pure
//! function of `(input, config, policy)`. The embedded policy package is the
//! single rule source.
//!
//! See also: ai-slop gates the prose a code repository ships, and
//! slop-detector reads text someone sent you.

pub mod engine;
pub mod extract;
pub mod input;
pub mod policy;
pub mod render;
pub mod report;
pub mod rules;
pub mod skill;
pub mod views;
pub mod waiver;

use std::ops::Range;

pub use report::{Coverage, Finding, Report};
pub use waiver::{Approval, VerifyOutcome, Waiver};

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Profile {
    Essay,
    BlogPost,
    Email,
    Report,
    Doc,
    SocialPost,
}

impl Profile {
    pub const ALL: [Profile; 6] = [
        Profile::Essay,
        Profile::BlogPost,
        Profile::Email,
        Profile::Report,
        Profile::Doc,
        Profile::SocialPost,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Essay => "essay",
            Profile::BlogPost => "blog-post",
            Profile::Email => "email",
            Profile::Report => "report",
            Profile::Doc => "doc",
            Profile::SocialPost => "social-post",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Profile> {
        Profile::ALL.iter().copied().find(|p| p.as_str() == s)
    }

    /// Index into the policy package profile_names order.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The input format the policy package declares for this profile.
    pub fn default_format(self) -> InputFormat {
        InputFormat::Markdown
    }

    /// Formats a caller may select for this profile.
    pub fn supported_formats(self) -> &'static [InputFormat] {
        &[InputFormat::Markdown, InputFormat::Text]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    Markdown,
    Text,
}

impl InputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            InputFormat::Markdown => "markdown",
            InputFormat::Text => "text",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<InputFormat> {
        match s {
            "markdown" => Some(InputFormat::Markdown),
            "text" => Some(InputFormat::Text),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WaiverAuthority {
    Human,
    OrchestratorAgent,
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_findings: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_bytes: 2 * 1024 * 1024,
            max_findings: 10_000,
        }
    }
}

/// Deployment configuration. All fields optional.
#[derive(Clone, Debug, Default)]
pub struct Deployment {
    pub waiver_authority: Option<WaiverAuthority>,
    pub demote: Vec<String>,
    pub expected_license_wording: Option<String>,
    pub exempt_comment_markers: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub profile: Profile,
    pub input_format: InputFormat,
    pub limits: Limits,
    pub suggest: bool,
    pub waivers: Vec<Waiver>,
    pub deployment: Deployment,
    /// Unix seconds used only for waiver expiry checks. None disables the
    /// expiry comparison, keeping `analyze` clock-free.
    pub now_unix: Option<i64>,
}

impl Config {
    pub fn new(profile: Profile) -> Config {
        Config {
            profile,
            input_format: profile.default_format(),
            limits: Limits::default(),
            suggest: false,
            waivers: Vec::new(),
            deployment: Deployment::default(),
            now_unix: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    /// Detector could not complete. Maps to exit 30.
    Instrumentation(String),
    /// Input outside the supported contract. Maps to exit 40.
    UnsupportedInput(String),
    /// Configuration misuse. Maps to exit 2.
    Usage(String),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::Instrumentation(m) => write!(f, "instrumentation_error: {m}"),
            AnalysisError::UnsupportedInput(m) => write!(f, "unsupported_input: {m}"),
            AnalysisError::Usage(m) => write!(f, "usage error: {m}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// A resolved rule stance for one profile and field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stance {
    Apply,
    Relax,
    Off,
}

/// Analyze one document. Never panics on any input.
pub fn analyze(input: &[u8], config: &Config) -> Result<Report, AnalysisError> {
    validate_config(config)?;
    let compiled = engine::compiled()
        .map_err(|e| AnalysisError::Instrumentation(format!("policy load failed: {e}")))?;
    let prepared = input::prepare(input, config)?;
    let doc = extract::build_doc(&prepared, config)?;
    // Every profile here reads prose, and a source file is not prose. Gating
    // one draws findings from statement punctuation rather than from writing,
    // so the boundary fails closed instead. The prose and code split is the
    // extractor's own: backtick fences, tilde fences, and indented blocks are
    // all code, so a document that quotes code stays a document. The test
    // reads Rust shape only. Source in another language reaches the rules and
    // produces findings a reader discounts.
    let code_blocks: Vec<Range<usize>> = doc
        .regions
        .iter()
        .filter(|r| r.kind == extract::RegionKind::CodeBlock)
        .map(|r| r.range.clone())
        .collect();
    if let Some((code_lines, nonblank)) = input::source_shape(&prepared.text, &code_blocks) {
        return Err(AnalysisError::UnsupportedInput(format!(
            "Input looks like a Rust source file: {code_lines} of {nonblank} lines \
             outside code blocks carry code structure. Pass the prose, or wrap \
             the code in a fenced block."
        )));
    }
    let norm = views::build_norm(&prepared.text, &doc);
    let mut hits = engine::scan_all(compiled, &prepared, &doc, &norm, config)?;
    rules::evaluate_structural(compiled, &prepared, &doc, &norm, config, &mut hits);
    report::assemble(compiled, &prepared, &doc, &norm, config, hits)
}

fn validate_config(config: &Config) -> Result<(), AnalysisError> {
    let compiled = engine::compiled()
        .map_err(|e| AnalysisError::Instrumentation(format!("policy load failed: {e}")))?;
    for id in &config.deployment.demote {
        let rule = compiled
            .pkg
            .rules
            .iter()
            .find(|r| &r.id == id)
            .ok_or_else(|| AnalysisError::Usage(format!("demote names unknown rule {id}")))?;
        if rule.id == "SLOP-J001" {
            return Err(AnalysisError::Usage(
                "SLOP-J001 is never demotable".to_string(),
            ));
        }
        if rule.tier != policy::Tier::Candidate {
            return Err(AnalysisError::Usage(format!(
                "demotion of non-candidate rule {id} is not allowed"
            )));
        }
    }
    // Waivers are span-bound and expiring. A span-less waiver would
    // blanket every finding of its rule and a non-expiring one would never
    // lapse, so both are rejected here — the single choke point covering the
    // CLI --waivers path and approval-embedded waivers alike.
    for w in &config.waivers {
        if w.span.is_none() || w.expires.is_none() {
            return Err(AnalysisError::Usage(format!(
                "waiver for {} must be span-bound and carry an explicit expires; \
                 a span-less or non-expiring waiver is invalid",
                w.rule_id
            )));
        }
    }
    if !config
        .profile
        .supported_formats()
        .contains(&config.input_format)
    {
        return Err(AnalysisError::Usage(format!(
            "format {} is not supported by profile {}",
            config.input_format.as_str(),
            config.profile.as_str()
        )));
    }
    Ok(())
}

/// Verify a payload against an approval record. `now_unix` is the caller's
/// clock reading as unix seconds.
pub fn verify(input: &[u8], approval: &Approval, now_unix: i64) -> VerifyOutcome {
    waiver::verify(input, approval, now_unix)
}

/// The sha256 digest of the canonicalized embedded policy package.
pub fn policy_digest() -> String {
    policy::compute_digest()
}

/// Clamp a byte range outward to char boundaries of `s`.
pub(crate) fn widen_to_char_boundaries(s: &str, mut r: Range<usize>) -> Range<usize> {
    if r.start > s.len() {
        r.start = s.len();
    }
    if r.end > s.len() {
        r.end = s.len();
    }
    while r.start > 0 && !s.is_char_boundary(r.start) {
        r.start -= 1;
    }
    while r.end < s.len() && !s.is_char_boundary(r.end) {
        r.end += 1;
    }
    if r.end < r.start {
        r.end = r.start;
    }
    r
}
