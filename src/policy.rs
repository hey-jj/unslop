//! Policy package loading, validation, and digest.
//!
//! The canonical package lives in `policy/` and is embedded at build time.
//! This module parses it into typed rules and computes the package digest
//! over a canonical serialization.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const POLICY_TOML: &str = include_str!("../policy/policy.toml");

/// Embedded lexicon files, keyed by their package-relative path.
pub const LEXICONS: &[(&str, &str)] = &[
    (
        "words/agent-loop.txt",
        include_str!("../policy/words/agent-loop.txt"),
    ),
    (
        "words/assistant-offers.txt",
        include_str!("../policy/words/assistant-offers.txt"),
    ),
    (
        "words/assistant-voice.txt",
        include_str!("../policy/words/assistant-voice.txt"),
    ),
    (
        "words/audience-runway.txt",
        include_str!("../policy/words/audience-runway.txt"),
    ),
    (
        "words/byline-valediction.txt",
        include_str!("../policy/words/byline-valediction.txt"),
    ),
    (
        "words/clarity-meta.txt",
        include_str!("../policy/words/clarity-meta.txt"),
    ),
    (
        "words/copula-avoidance.txt",
        include_str!("../policy/words/copula-avoidance.txt"),
    ),
    (
        "words/correspondence-offers.txt",
        include_str!("../policy/words/correspondence-offers.txt"),
    ),
    (
        "words/courtesy-closings.txt",
        include_str!("../policy/words/courtesy-closings.txt"),
    ),
    (
        "words/cutoff-disclaimers.txt",
        include_str!("../policy/words/cutoff-disclaimers.txt"),
    ),
    (
        "words/decorative-diction.txt",
        include_str!("../policy/words/decorative-diction.txt"),
    ),
    (
        "words/empty-qualifiers.txt",
        include_str!("../policy/words/empty-qualifiers.txt"),
    ),
    (
        "words/era-overuse.txt",
        include_str!("../policy/words/era-overuse.txt"),
    ),
    (
        "words/filler-meta.txt",
        include_str!("../policy/words/filler-meta.txt"),
    ),
    (
        "words/formulaic-challenges.txt",
        include_str!("../policy/words/formulaic-challenges.txt"),
    ),
    (
        "words/generic-conclusions.txt",
        include_str!("../policy/words/generic-conclusions.txt"),
    ),
    (
        "words/first-person.txt",
        include_str!("../policy/words/first-person.txt"),
    ),
    (
        "words/hype-adjectives.txt",
        include_str!("../policy/words/hype-adjectives.txt"),
    ),
    (
        "words/impact-framing.txt",
        include_str!("../policy/words/impact-framing.txt"),
    ),
    (
        "words/importance-adjectives.txt",
        include_str!("../policy/words/importance-adjectives.txt"),
    ),
    (
        "words/inflated-diction.txt",
        include_str!("../policy/words/inflated-diction.txt"),
    ),
    (
        "words/injection.txt",
        include_str!("../policy/words/injection.txt"),
    ),
    (
        "words/intensifiers.txt",
        include_str!("../policy/words/intensifiers.txt"),
    ),
    (
        "words/magnitude-claims.txt",
        include_str!("../policy/words/magnitude-claims.txt"),
    ),
    (
        "words/metaphor-nouns.txt",
        include_str!("../policy/words/metaphor-nouns.txt"),
    ),
    (
        "words/ornamental.txt",
        include_str!("../policy/words/ornamental.txt"),
    ),
    (
        "words/participial-tails.txt",
        include_str!("../policy/words/participial-tails.txt"),
    ),
    (
        "words/pleasantries.txt",
        include_str!("../policy/words/pleasantries.txt"),
    ),
    (
        "words/plain-word-swaps.txt",
        include_str!("../policy/words/plain-word-swaps.txt"),
    ),
    (
        "words/promotional.txt",
        include_str!("../policy/words/promotional.txt"),
    ),
    (
        "words/puffery.txt",
        include_str!("../policy/words/puffery.txt"),
    ),
    (
        "words/provenance-oblique.txt",
        include_str!("../policy/words/provenance-oblique.txt"),
    ),
    (
        "words/provider-artifacts.txt",
        include_str!("../policy/words/provider-artifacts.txt"),
    ),
    (
        "words/provider-attribution.txt",
        include_str!("../policy/words/provider-attribution.txt"),
    ),
    (
        "words/reassurance.txt",
        include_str!("../policy/words/reassurance.txt"),
    ),
    (
        "words/signature-lines.txt",
        include_str!("../policy/words/signature-lines.txt"),
    ),
    (
        "words/softeners.txt",
        include_str!("../policy/words/softeners.txt"),
    ),
    (
        "words/significance-inflation.txt",
        include_str!("../policy/words/significance-inflation.txt"),
    ),
    (
        "words/stock-openers.txt",
        include_str!("../policy/words/stock-openers.txt"),
    ),
    (
        "words/tracking-params.txt",
        include_str!("../policy/words/tracking-params.txt"),
    ),
    (
        "words/transition-openers.txt",
        include_str!("../policy/words/transition-openers.txt"),
    ),
    (
        "words/vague-attribution.txt",
        include_str!("../policy/words/vague-attribution.txt"),
    ),
    (
        "words/verification-claims.txt",
        include_str!("../policy/words/verification-claims.txt"),
    ),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Violation,
    Candidate,
    CoverageHint,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Violation => "violation",
            Tier::Candidate => "candidate",
            Tier::CoverageHint => "coverage_hint",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lifecycle {
    Blocking,
    Advisory,
    Experimental,
    Deprecated,
}

impl Lifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Lifecycle::Blocking => "blocking",
            Lifecycle::Advisory => "advisory",
            Lifecycle::Experimental => "experimental",
            Lifecycle::Deprecated => "deprecated",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Raw,
    Prose,
    Norm,
    Rendered,
}

impl View {
    pub fn as_str(self) -> &'static str {
        match self {
            View::Raw => "raw",
            View::Prose => "prose",
            View::Norm => "norm",
            View::Rendered => "rendered",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MatchKindSpec {
    WordSet,
    RegexSet,
    Structural,
    Ratio,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    None,
    LinkUrl,
    Code,
    Heading,
    Comment,
}

use crate::Stance;

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub family: String,
    pub tier: Tier,
    pub lifecycle: Lifecycle,
    pub origin: String,
    pub human_only_waiver: bool,
    pub view: View,
    pub kind: MatchKindSpec,
    pub lexicon: Option<String>,
    /// Resolved literal terms (lexicon entries plus inline words).
    pub terms: Vec<String>,
    /// Plain replacements for the terms whose lexicon line carried one,
    /// keyed by the lowercased term.
    pub swaps: BTreeMap<String, String>,
    pub case_sensitive: bool,
    pub boundary_word: bool,
    pub block_start: bool,
    pub scope: Scope,
    pub params: toml::Value,
    pub patterns: Vec<String>,
    pub guard: String,
    pub judge: Option<String>,
    /// Indexed by profile order in `[semantics].profile_names`.
    pub stances: Vec<Stance>,
    /// Exemption collocations. Any phrase covering a match suppresses it.
    pub exemptions: Vec<String>,
}

impl Rule {
    pub fn stance(&self, profile: crate::Profile) -> Stance {
        self.stances[profile.index()]
    }

    /// Active-profile bitmask, one bit per profile in package order.
    pub fn profile_mask(&self) -> u8 {
        let mut mask = 0u8;
        for (i, st) in self.stances.iter().enumerate() {
            if *st != Stance::Off {
                mask |= 1 << i;
            }
        }
        mask
    }
}

#[derive(Clone, Debug)]
pub struct ProfileDef {
    pub name: String,
    pub format: String,
    pub core_rules: BTreeMap<String, String>,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct PolicyPackage {
    pub version: String,
    pub digest: String,
    pub quotation_downgrade: Vec<String>,
    /// Rules whose hits inside claimed-quotation regions are dropped at
    /// report resolution rather than downgraded. A candidate-tier rule has
    /// no lower blocking state, so suppression is the quotation semantics
    /// that fits it.
    pub quotation_suppress: Vec<String>,
    pub profile_names: Vec<String>,
    pub profiles: Vec<ProfileDef>,
    pub rules: Vec<Rule>,
}

impl PolicyPackage {
    pub fn rule_by_id(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

/// Canonical serialization for the digest: the policy.toml body with the
/// digest value emptied and CRLF folded to LF, followed by each lexicon file
/// (path-sorted), each preceded by a NUL-delimited path header.
pub fn canonical_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"policy.toml\0");
    let toml_lf = POLICY_TOML.replace("\r\n", "\n");
    for line in toml_lf.split_inclusive('\n') {
        if line.trim_start().starts_with("digest = ") {
            out.extend_from_slice(b"digest = \"\"\n");
        } else {
            out.extend_from_slice(line.as_bytes());
        }
    }
    let mut files: Vec<(&str, &str)> = LEXICONS.to_vec();
    files.sort_by_key(|(p, _)| *p);
    for (path, content) in files {
        out.push(0);
        out.extend_from_slice(path.as_bytes());
        out.push(0);
        out.extend_from_slice(content.replace("\r\n", "\n").as_bytes());
    }
    out
}

pub fn compute_digest() -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Parse one lexicon into its terms and, for any line written as
/// `inflated -> plain`, the replacement the plain half supplies. The
/// replacement reaches the finding as a suggested fix, so a lexicon that
/// knows the edit ships the edit.
fn lexicon_terms(path: &str) -> Result<(Vec<String>, BTreeMap<String, String>), String> {
    let (_, content) = LEXICONS
        .iter()
        .find(|(p, _)| *p == path)
        .ok_or_else(|| format!("lexicon {path} is not embedded"))?;
    let mut terms = Vec::new();
    let mut swaps = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once(" -> ") {
            Some((from, to)) => {
                let from = from.trim();
                let to = to.trim();
                if from.is_empty() || to.is_empty() {
                    return Err(format!("lexicon {path}: malformed swap line {line}"));
                }
                terms.push(from.to_string());
                swaps.insert(from.to_lowercase(), to.to_string());
            }
            None => terms.push(line.to_string()),
        }
    }
    if terms.is_empty() {
        return Err(format!("lexicon {path} has no terms"));
    }
    Ok((terms, swaps))
}

fn as_str(v: &toml::Value, what: &str) -> Result<String, String> {
    v.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("{what} must be a string"))
}

fn parse_stance(s: &str, what: &str) -> Result<Stance, String> {
    match s {
        "apply" => Ok(Stance::Apply),
        "relax" => Ok(Stance::Relax),
        "off" => Ok(Stance::Off),
        other => Err(format!("{what}: unknown stance {other}")),
    }
}

/// Parse a rule's optional `exemptions` table into one flat list of covering
/// phrases. Every shape that would silently exempt nothing fails the load
/// instead: a value that is not a table, a table with no keys, a key whose
/// value is not an array, an empty array, and an empty phrase. A quietly dead
/// exemption is worse than a missing one, because the guard text goes on
/// promising it.
fn parse_exemptions(rt: &toml::value::Table, id: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let Some(v) = rt.get("exemptions") else {
        return Ok(out);
    };
    let table = v
        .as_table()
        .ok_or_else(|| format!("rule {id}: exemptions must be a table"))?;
    if table.is_empty() {
        return Err(format!("rule {id}: exemptions is an empty table"));
    }
    for (term, phrases) in table {
        let arr = phrases
            .as_array()
            .ok_or_else(|| format!("rule {id}: exemptions.{term} must be an array of phrases"))?;
        if arr.is_empty() {
            return Err(format!("rule {id}: exemptions.{term} is an empty array"));
        }
        for phrase in arr {
            let p = as_str(phrase, "exemption phrase")?;
            if p.trim().is_empty() {
                return Err(format!("rule {id}: exemptions.{term} has an empty phrase"));
            }
            out.push(p.to_lowercase());
        }
    }
    Ok(out)
}

/// Parse the embedded package. Returns an error string on any structural
/// defect. Callers surface this as `instrumentation_error`.
pub fn load() -> Result<PolicyPackage, String> {
    let root: toml::Value =
        toml::from_str(POLICY_TOML).map_err(|e| format!("policy.toml parse: {e}"))?;
    let table = root.as_table().ok_or("policy.toml root is not a table")?;

    let policy_tbl = table
        .get("policy")
        .and_then(|v| v.as_table())
        .ok_or("[policy] missing")?;
    let version = policy_tbl
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or("[policy].version missing")?
        .to_string();

    let semantics = table
        .get("semantics")
        .and_then(|v| v.as_table())
        .ok_or("[semantics] missing")?;
    let quotation_downgrade = semantics
        .get("quotation_downgrade")
        .and_then(|v| v.as_array())
        .ok_or("[semantics].quotation_downgrade missing")?
        .iter()
        .map(|v| as_str(v, "quotation_downgrade entry"))
        .collect::<Result<Vec<_>, _>>()?;
    let quotation_suppress = semantics
        .get("quotation_suppress")
        .and_then(|v| v.as_array())
        .ok_or("[semantics].quotation_suppress missing")?
        .iter()
        .map(|v| as_str(v, "quotation_suppress entry"))
        .collect::<Result<Vec<_>, _>>()?;
    let profile_names = semantics
        .get("profile_names")
        .and_then(|v| v.as_array())
        .ok_or("[semantics].profile_names missing")?
        .iter()
        .map(|v| as_str(v, "profile_names entry"))
        .collect::<Result<Vec<_>, _>>()?;
    if profile_names.len() != crate::Profile::ALL.len() {
        return Err(format!(
            "expected {} profiles, found {}",
            crate::Profile::ALL.len(),
            profile_names.len()
        ));
    }
    for (i, p) in crate::Profile::ALL.iter().enumerate() {
        if profile_names[i] != p.as_str() {
            return Err(format!(
                "profile order mismatch at {i}: package says {}, crate says {}",
                profile_names[i],
                p.as_str()
            ));
        }
    }

    let profile_tbl = table
        .get("profile")
        .and_then(|v| v.as_table())
        .ok_or("[profile.*] missing")?;
    let mut profiles = Vec::new();
    for name in &profile_names {
        let def = profile_tbl
            .get(name)
            .and_then(|v| v.as_table())
            .ok_or_else(|| format!("[profile.{name}] missing"))?;
        let format = def
            .get("format")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("[profile.{name}].format missing"))?
            .to_string();
        let mut core_rules = BTreeMap::new();
        if let Some(cr) = def.get("core_rules").and_then(|v| v.as_table()) {
            for (k, v) in cr {
                core_rules.insert(k.clone(), as_str(v, "core rule stance")?);
            }
        }
        let notes = def
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        profiles.push(ProfileDef {
            name: name.clone(),
            format,
            core_rules,
            notes,
        });
    }

    let rules_arr = table
        .get("rule")
        .and_then(|v| v.as_array())
        .ok_or("[[rule]] entries missing")?;
    let mut rules = Vec::new();
    let mut lexicon_uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rv in rules_arr {
        let rt = rv.as_table().ok_or("rule entry is not a table")?;
        let id = rt
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("rule missing id")?
            .to_string();
        let get_str = |k: &str| -> Result<String, String> {
            rt.get(k)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| format!("rule {id} missing {k}"))
        };
        let name = get_str("name")?;
        let family = get_str("family")?;
        let tier = match get_str("tier")?.as_str() {
            "violation" => Tier::Violation,
            "candidate" => Tier::Candidate,
            "coverage_hint" => Tier::CoverageHint,
            other => return Err(format!("rule {id}: unknown tier {other}")),
        };
        let lifecycle = match get_str("lifecycle")?.as_str() {
            "blocking" => Lifecycle::Blocking,
            "advisory" => Lifecycle::Advisory,
            "experimental" => Lifecycle::Experimental,
            "deprecated" => Lifecycle::Deprecated,
            other => return Err(format!("rule {id}: unknown lifecycle {other}")),
        };
        let origin = get_str("origin")?;
        let human_only_waiver = rt
            .get("waiver")
            .and_then(|v| v.as_str())
            .map(|s| s == "human-only")
            .unwrap_or(false);
        let view = match get_str("view")?.as_str() {
            "raw" => View::Raw,
            "prose" => View::Prose,
            "norm" => View::Norm,
            "rendered" => View::Rendered,
            other => return Err(format!("rule {id}: unknown view {other}")),
        };
        let match_tbl = rt
            .get("match")
            .and_then(|v| v.as_table())
            .ok_or_else(|| format!("rule {id} missing match"))?;
        let kind = match match_tbl.get("kind").and_then(|v| v.as_str()) {
            Some("word-set") => MatchKindSpec::WordSet,
            Some("regex-set") => MatchKindSpec::RegexSet,
            Some("structural") => MatchKindSpec::Structural,
            Some("ratio") => MatchKindSpec::Ratio,
            other => return Err(format!("rule {id}: bad match kind {other:?}")),
        };
        let lexicon = match_tbl
            .get("lexicon")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mut terms = Vec::new();
        let mut swaps = BTreeMap::new();
        if let Some(path) = &lexicon {
            lexicon_uses
                .entry(path.clone())
                .or_default()
                .push(id.clone());
            let (t, sw) = lexicon_terms(path)?;
            terms.extend(t);
            swaps.extend(sw);
        }
        if let Some(words) = match_tbl.get("words").and_then(|v| v.as_array()) {
            for w in words {
                terms.push(as_str(w, "match.words entry")?);
            }
        }
        if kind == MatchKindSpec::WordSet && terms.is_empty() {
            return Err(format!("word-set rule {id} has no terms"));
        }
        let case_sensitive = match match_tbl.get("case").and_then(|v| v.as_str()) {
            Some("sensitive") => true,
            Some("insensitive") | None => false,
            Some(other) => return Err(format!("rule {id}: unknown case mode {other}")),
        };
        let boundary_word = match match_tbl.get("boundary").and_then(|v| v.as_str()) {
            Some("word") => true,
            Some("none") | None => false,
            Some(other) => return Err(format!("rule {id}: unknown boundary {other}")),
        };
        let block_start = match match_tbl.get("position").and_then(|v| v.as_str()) {
            Some("block-start") => true,
            None => false,
            Some(other) => return Err(format!("rule {id}: unknown position {other}")),
        };
        let scope = match match_tbl.get("scope").and_then(|v| v.as_str()) {
            Some("link-url") => Scope::LinkUrl,
            Some("code") => Scope::Code,
            Some("heading") => Scope::Heading,
            Some("comment") => Scope::Comment,
            None => Scope::None,
            Some(other) => return Err(format!("rule {id}: unknown scope {other}")),
        };
        let params = match_tbl
            .get("params")
            .cloned()
            .unwrap_or(toml::Value::Table(Default::default()));
        let mut patterns = Vec::new();
        if let Some(pats) = rt.get("patterns").and_then(|v| v.as_array()) {
            for p in pats {
                patterns.push(as_str(p, "patterns entry")?);
            }
        }
        if kind == MatchKindSpec::RegexSet && patterns.is_empty() {
            return Err(format!("regex-set rule {id} has no patterns"));
        }
        let guard = get_str("guard")?;
        let judge = rt
            .get("judge")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if tier == Tier::Candidate && judge.is_none() {
            return Err(format!("candidate rule {id} has no judge question"));
        }

        let profiles_val = rt
            .get("profiles")
            .and_then(|v| v.as_table())
            .ok_or_else(|| format!("rule {id} missing profiles"))?;
        let stance_of = |v: &toml::Value, what: String| -> Result<Stance, String> {
            match v {
                toml::Value::String(s) => parse_stance(s, &what),
                _ => Err(format!("{what} must be a string")),
            }
        };
        let default_stance = match profiles_val.get("default") {
            Some(v) => stance_of(v, format!("rule {id} profiles.default"))?,
            None => Stance::Apply,
        };
        let mut stances = vec![default_stance; crate::Profile::ALL.len()];
        for (k, v) in profiles_val {
            if k == "default" {
                continue;
            }
            let idx = profile_names
                .iter()
                .position(|n| n == k)
                .ok_or_else(|| format!("rule {id}: unknown profile {k}"))?;
            stances[idx] = stance_of(v, format!("rule {id} profiles.{k}"))?;
        }

        let exemptions = parse_exemptions(rt, &id)?;

        rules.push(Rule {
            id,
            name,
            family,
            tier,
            lifecycle,
            origin,
            human_only_waiver,
            view,
            kind,
            lexicon,
            terms,
            swaps,
            case_sensitive,
            boundary_word,
            block_start,
            scope,
            params,
            patterns,
            guard,
            judge,
            stances,
            exemptions,
        });
    }

    // Every embedded lexicon must be referenced by exactly one rule.
    for (path, _) in LEXICONS {
        match lexicon_uses.get(*path).map(|v| v.len()).unwrap_or(0) {
            1 => {}
            0 => return Err(format!("lexicon {path} is referenced by no rule")),
            n => return Err(format!("lexicon {path} is referenced by {n} rules")),
        }
    }

    let mut seen = std::collections::BTreeSet::new();
    for r in &rules {
        if !seen.insert(r.id.clone()) {
            return Err(format!("duplicate rule id {}", r.id));
        }
    }

    // A typo in a quotation-semantics list would silently no-op; fail loud.
    for id in quotation_downgrade.iter().chain(quotation_suppress.iter()) {
        if !rules.iter().any(|r| &r.id == id) {
            return Err(format!("quotation semantics list names unknown rule {id}"));
        }
    }

    let digest = compute_digest();
    Ok(PolicyPackage {
        version,
        digest,
        quotation_downgrade,
        quotation_suppress,
        profile_names,
        profiles,
        rules,
    })
}

#[cfg(test)]
mod exemption_table_tests {
    use super::parse_exemptions;

    fn rule_table(src: &str) -> toml::value::Table {
        toml::from_str::<toml::Value>(src)
            .unwrap()
            .as_table()
            .unwrap()
            .clone()
    }

    #[test]
    fn a_populated_table_loads_and_lowercases() {
        let rt = rule_table("[exemptions]\nport = [\"Serial Port\"]\n");
        assert_eq!(
            parse_exemptions(&rt, "SLOP-TEST").unwrap(),
            vec!["serial port".to_string()]
        );
    }

    #[test]
    fn no_table_is_no_exemptions() {
        let rt = rule_table("id = \"SLOP-TEST\"\n");
        assert!(parse_exemptions(&rt, "SLOP-TEST").unwrap().is_empty());
    }

    /// Each of these would exempt nothing while the guard text says it does.
    #[test]
    fn unmatchable_tables_fail_the_load() {
        for src in [
            "exemptions = \"port\"\n",
            "[exemptions]\n",
            "[exemptions]\nport = \"serial port\"\n",
            "[exemptions]\nport = []\n",
            "[exemptions]\nport = [\"\"]\n",
            "[exemptions]\nport = [\"   \"]\n",
        ] {
            let rt = rule_table(src);
            assert!(
                parse_exemptions(&rt, "SLOP-TEST").is_err(),
                "loaded a dead exemption table: {src:?}"
            );
        }
    }
}
