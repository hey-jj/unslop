//! Section 12.3: `source[start..end] == snippet` for every finding on every
//! input, segmentation total coverage, duplicate substrings, nested
//! markdown, multibyte and CRLF cases.

mod common;

use common::{assert_invariants, run};
use unslop::{analyze, Config, Profile};

const INPUTS: &[&str] = &[
    "We delve into things.\n",
    "Plain text, no markdown constructs at all.",
    "# Head\n\nA — dash and a ; semicolon.\n\n```rust\nlet x = 1; // delve\n```\n",
    "CRLF line one.\r\nWe delve here.\r\n\r\nMore.\r\n",
    "Entity dash &mdash; here and &#8212; there.\n",
    "Zero width de\u{200B}lve evasion.\n",
    "Multibyte céfé — naïve delve résumé.\n",
    "> quoted; semicolon and — dash inside a blockquote\n",
    "- item one with *emphasis*\n- [link text](https://example.com/?utm_source=chatgpt&x=1)\n",
    "delve delve delve\n\ndelve again delve\n",
    "Nested [link with *emph inside* text](https://e.io) in a list:\n\n- > quote with **bold delve** here\n",
    "| a | b |\n|---|---|\n| delve | — |\n",
    "Footnote[^1] text.\n\n[^1]: the note has a — dash\n",
    "Autolink <https://example.com/?utm_source=chatgpt.com> in prose.\n",
    "Heading *with emph*\n===\n\nbody\n",
    "Term split across a soft\nbreak: game\nchanger indeed.\n",
    "An i\u{0301}nput with combining marks and delve.\n",
    "*This* is **not** what it seems, but rather — well.\n",
    // A payload opening on a multi-byte character. A rule that reports the
    // document as a whole anchors on the first character, and a one-byte
    // anchor landed mid-character here and failed the run closed.
    "\u{1F389} The rule reports the span, not the sentence.\n",
    "— The rule reports the span, not the sentence.\n",
    "é The rule reports the span, not the sentence.\n",
];

#[test]
fn span_invariant_over_inputs() {
    for text in INPUTS {
        for profile in [Profile::Doc, Profile::Doc, Profile::GeneralWriting] {
            let report = run(text, profile);
            assert_invariants(text, &report);
        }
    }
}

/// A rule that speaks about the document rather than about a place in it
/// still reports a span, and that span is the first character of the payload.
/// One byte was the old anchor, which lands mid-character on any payload
/// opening outside ASCII and failed the whole run closed with exit 30. The
/// three openings here are the ones a writer actually types.
#[test]
fn a_whole_document_span_opens_on_a_whole_character() {
    for (label, text) in [
        (
            "emoji",
            "\u{1F389} The rule reports the span, not the sentence.\n",
        ),
        (
            "em dash",
            "— The rule reports the span, not the sentence.\n",
        ),
        (
            "accented letter",
            "é The rule reports the span, not the sentence.\n",
        ),
    ] {
        let report = analyze(text.as_bytes(), &Config::new(Profile::Doc))
            .unwrap_or_else(|e| panic!("{label}-first payload failed the run: {e}"));
        assert_invariants(text, &report);
        // The contrast-density hint speaks about the document, so it is the
        // one that carries the anchor here.
        let whole = report
            .findings
            .iter()
            .find(|f| f.rule_id == "SLOP-C009")
            .unwrap_or_else(|| panic!("{label}: no document-anchored finding to check"));
        let span = &whole.spans[0];
        assert_eq!(span.start, 0, "{label}");
        let first = text.chars().next().unwrap();
        assert_eq!(
            span.end,
            first.len_utf8(),
            "{label}: the anchor is the whole first character"
        );
        assert!(text.is_char_boundary(span.end), "{label}");
    }
}

/// The block-start fix made SLOP-D004 reachable behind decoration, since it
/// counts SLOP-T002 hits and those now report behind a leading emoji. The
/// document-anchored span it carries is what the crash used to hide.
#[test]
fn the_opener_density_rule_is_reachable_behind_decoration() {
    let decorated = "\u{1F389} Moreover, one thing happened.\n\n\
                     \u{1F389} Furthermore, a second thing happened.\n\n\
                     \u{1F389} Additionally, a third thing happened.\n";
    let report = analyze(decorated.as_bytes(), &Config::new(Profile::Doc))
        .expect("a decorated opener document must not fail the run");
    assert_invariants(decorated, &report);
    let t002 = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-T002")
        .count();
    assert_eq!(t002, 3, "each decorated opener reports");
    let d004 = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-D004")
        .expect("three openers reach the density threshold");
    assert_eq!(d004.spans[0].start, 0);
    assert_eq!(d004.spans[0].end, '\u{1F389}'.len_utf8());
}

#[test]
fn duplicate_substring_occurrences_get_distinct_spans() {
    let text = "delve one delve two delve three\n";
    let report = run(text, Profile::Doc);
    let spans: Vec<(usize, usize)> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A001")
        .map(|f| (f.spans[0].start, f.spans[0].end))
        .collect();
    assert_eq!(spans, vec![(0, 5), (10, 15), (20, 25)]);
    for (s, e) in spans {
        assert_eq!(&text[s..e], "delve");
    }
}

#[test]
fn the_text_format_holds_invariants() {
    let mut config = unslop::Config::new(Profile::GeneralWriting);
    config.input_format = unslop::InputFormat::Text;
    let text = "plain text with — a dash\nand a second; line\n";
    let report = unslop::analyze(text.as_bytes(), &config).unwrap();
    assert_invariants(text, &report);
}

#[test]
fn bom_offsets_index_the_post_bom_payload() {
    let payload = "\u{FEFF}We delve here.\n";
    let stripped = &payload[3..];
    let report = unslop::analyze(payload.as_bytes(), &unslop::Config::new(Profile::Doc)).unwrap();
    assert_invariants(stripped, &report);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .expect("delve fires");
    assert_eq!(&stripped[f.spans[0].start..f.spans[0].end], "delve");
    // The document hash covers the original bytes as received.
    assert_eq!(report.document.bytes, payload.len());
}
