//! Regression coverage for heading-span coordinates, fence-close indentation,
//! numeric-reference decoding, and the HTML fail-closed anomaly set:
//!
//!   - the heading scope does not widen a heading-LOCAL span against the
//!     full source: a non-ASCII byte before a heading cannot drag a
//!     heading finding's span end +1.
//!   - a closing fence's indentation is capped at 3 spaces (CommonMark), so a
//!     4-space-indented ``` at EOF does NOT close the block — the slop tail
//!     fails closed instead of being swallowed as code.
//!   - numeric character references (`&#DDD;` / `&#xHH;`) decode
//!     arithmetically, so `&#100;elve` cannot hide a word.
//!   - a hyphen is part of a tag name, so `<code-sample>` parses as an
//!     ordinary unknown block element whose body IS scanned.
//!   - the enumerated constructs the hand-rolled tokenizer cannot render-
//!     faithfully parse (`<![CDATA[` outside a comment, `--!>`, a self-closing
//!     SKIP_BODY element, `<template>`) fail closed as an M005 anomaly, while
//!     ordinary custom elements and realistic HTML do NOT anomaly-flag.
//!   - the raw-HTML-dominance metric counts only genuine markup bytes: the
//!     reader-visible HTML text scanned as prose is subtracted, so an
//!     idiomatic HTML-using README does not trip the 20% threshold.

mod common;

use common::{has_rule, rule_ids, run, snippet, violations};
use unslop::Profile;

// Ordinary prose that dominates the byte count so the raw-HTML dominance branch
// of M005 is never what fires in the HTML cases below.
fn filler() -> String {
    "This paragraph is ordinary visible prose that carries the document. ".repeat(16)
}

// Heading-span coordinates --------------------------------------------------

#[test]
fn non_ascii_before_a_heading_keeps_the_exact_word_span() {
    // The document begins with five single-byte chars then a two-byte char
    // (İ at bytes 5..7), so a span widened in the wrong coordinate system
    // lands mid-char and drags the reported slice past the matched word.
    let text = "aaaaaİ ordinary words to fill this opening line here.\n\n# Impact now\n\nThe body describes what happened in plain prose for the reader.\n";
    let report = run(text, Profile::Doc);
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-F003")
        .expect("impact-framing must fire on the Impact heading");
    assert_eq!(snippet(f), "Impact", "span must be exactly the word");
    assert_eq!(f.spans[0].end - f.spans[0].start, 6);
}

// Fence-close indent cap -------------------------------------------------

#[test]
fn fence_close_indented_four_spaces_does_not_close() {
    // A ```-opened fence, code, a slop line, then a 4-space-indented ``` at EOF.
    // CommonMark caps a closing fence at 3 spaces, so this does NOT close: the
    // block runs to EOF unclosed and M005 fires (the slop tail is not swallowed).
    let text = format!(
        "{f}\n\n```\ncode\ndelve game-changer\n    ```\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "a 4-space-indented close must fail closed: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn fence_close_indented_three_spaces_still_closes() {
    // The same shape with a 3-space indent: a valid close. The fence closes, so
    // the trailing "delve game-changer" is ordinary prose (A001) and there is no
    // unclosed-fence M005.
    let text = format!(
        "{f}\n\n```\ncode\n   ```\n\ndelve game-changer here.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a 3-space-indented close must still close: {:?}",
        rule_ids(&report)
    );
    assert!(
        has_rule(&report, "SLOP-A001"),
        "the tail after a closed fence is prose: {:?}",
        rule_ids(&report)
    );
}

// Numeric character reference decode --------------------------------------

#[test]
fn numeric_entity_evasions_are_decoded_and_flagged() {
    for src in [
        "<div>ordinary &#100;elve detail</div>",
        "<div>ordinary d&#x65;lve detail</div>",
        "<div>ordinary game&#32;changer detail</div>",
    ] {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-A001"),
            "numeric-ref-hidden slop must fire A001 for {src:?}: {:?}",
            rule_ids(&report)
        );
    }
}

#[test]
fn bare_or_malformed_numeric_ref_stays_literal() {
    // A bare `&#` and a non-digit ref carry no hidden word and must not crash or
    // spuriously decode into a slop match.
    let text = format!(
        "{f}\n\nA bare &# and a &#zz; reference stay literal.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "no slop word should appear: {:?}",
        rule_ids(&report)
    );
}

// Hyphenated custom-element tag names -------------------------------------

#[test]
fn hyphenated_custom_element_body_is_scanned() {
    // `<code-sample>` must not be misread as `<code>` (whose body is skipped as
    // code); its body is ordinary visible text and IS scanned.
    let text = format!(
        "{f}\n\n<div><code-sample>delve game-changer</code-sample></div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "custom-element body must be scanned: {:?}",
        rule_ids(&report)
    );
}

// Enumerated unparseable constructs fail closed ------------------------------

#[test]
fn unparseable_constructs_fail_closed_as_anomaly() {
    let cases = [
        (
            "cdata outside comment",
            "<svg><text><![CDATA[ delve ]]></text></svg>",
        ),
        (
            "malformed comment terminator",
            "<div><!-- hidden --!> visible --><p>delve</p></div>",
        ),
        (
            "self-closing skip-body",
            "<div><script/>delve game-changer</script></div>",
        ),
        (
            "template element",
            "<template>delve game-changer</template>",
        ),
    ];
    for (label, src) in cases {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-M005"),
            "{label} must fail closed via M005: {:?}",
            rule_ids(&report)
        );
    }
}

// Guardrail — ordinary custom elements and realistic HTML do NOT flag,
// and slop inside them is still scanned.

#[test]
fn ordinary_elements_do_not_anomaly_flag() {
    let benign = [
        "<my-widget>ordinary content here</my-widget>",
        "<x-foo>ordinary content here</x-foo>",
        "<details><summary>More</summary>ordinary details body here</details>",
        "<table>\n<tr><td>cell one text</td><td>cell two text</td></tr>\n</table>",
        "Inline <i>italic</i> and <b>bold</b> and <code>snippet</code> text.",
        "<div align=\"center\"><img src=\"x.png\"/></div>",
        // CDATA INSIDE a comment must not trip the CDATA anomaly.
        "<!-- <![CDATA[ x ]]> harmless -->",
    ];
    for src in benign {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            !has_rule(&report, "SLOP-M005"),
            "benign HTML must NOT anomaly-flag for {src:?}: {:?}",
            rule_ids(&report)
        );
    }
}

#[test]
fn slop_inside_ordinary_custom_element_is_still_scanned() {
    let text = format!(
        "{f}\n\n<my-widget>ordinary delve content</my-widget>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001") && !has_rule(&report, "SLOP-M005"),
        "custom element: slop scanned, no anomaly: {:?}",
        rule_ids(&report)
    );
}

// Tab-indented list-item comment is not split --------------------------

#[test]
fn tab_indented_list_item_comment_is_not_split() {
    // pulldown emits a zero-range whitespace Text event between the wrapped
    // lines of this tab-indented comment; unguarded it split the comment,
    // hiding it from Y001 and leaking the tail (delve/game-changer) to a prose
    // scan (A001) plus a spurious unclosed-comment M005. Guarded, the comment
    // stays whole: Y001 fires, no M005, no A001 leak.
    let text = format!(
        "{f}\n\n- <!-- ordinary hidden note\n\tdelve game-changer -->\n- visible item\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-Y001"),
        "comment recognized: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "no spurious anomaly: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "no leaked comment tail: {:?}",
        rule_ids(&report)
    );
}

// M005 raw-HTML-dominance double-count --------------------------------------

#[test]
fn html_visible_text_is_not_counted_toward_raw_dominance() {
    // A doc dominated by an HTML feature table whose cells are mostly visible
    // text. Counting the whole table toward raw-HTML dominance would trip
    // the 20% threshold; counting only the tag bytes keeps it under, so a
    // no-slop table doc passes.
    let text = "# Feature Matrix\n\nA short intro paragraph that explains the project in plain prose here.\n\n<table>\n<tr><td>Reads the input file carefully and validates every record before use</td></tr>\n<tr><td>Writes the output records in a stable deterministic order on every run</td></tr>\n<tr><td>Handles many of the common text formats that projects already use daily</td></tr>\n<tr><td>Runs on every supported platform without extra configuration or setup work</td></tr>\n<tr><td>Reports clear errors with the exact line and column where a problem occurs</td></tr>\n<tr><td>Ships with thorough documentation and a friendly quick start guide for you</td></tr>\n</table>\n\n## Closing\n\nA short closing paragraph in plain prose so the document is not entirely table.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "an HTML table of visible text must not trip raw-HTML dominance: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn genuinely_raw_markup_heavy_doc_still_trips_dominance() {
    // Almost pure tag markup with little visible text: still dominated by raw
    // HTML, so M005 must still fire. An absolute net-markup
    // floor (800 bytes) sits alongside the 20% ratio, so this doc carries
    // enough markup to clear the floor while staying nearly text-free.
    let row = "<div class=\"b\" data-y=\"2\" role=\"presentation\" style=\"color:red;padding:4px;margin:2px;border:1px solid black\"></div>\n";
    let text = format!(
        "<div class=\"a\" data-x=\"1\" id=\"root\" role=\"main\" aria-label=\"container\" style=\"display:flex\">\n{}</div>\n",
        row.repeat(8)
    );
    assert!(text.len() > 800, "fixture must clear the byte floor");
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "a raw-markup-heavy doc must still trip dominance: {:?}",
        rule_ids(&report)
    );
    let _ = violations(&report);
}
