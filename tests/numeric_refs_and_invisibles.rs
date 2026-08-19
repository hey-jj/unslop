//! Regression coverage for numeric-reference classification, column-based
//! fence-close measurement, nested hidden-content triggers, tag-syntax edge
//! cases, and the dominance floor:
//!
//!   - numeric character references are CLASSIFIED, not merely decoded:
//!     ordinary printables decode through the same pipeline literal chars
//!     get; references to invisible/control codepoints are elided to
//!     U+FFFD and fail closed as SLOP-M005; browser-decodable forms
//!     outside the CommonMark grammar (overlong, semicolonless) fail
//!     closed in HTML-derived text and stay inert in markdown.
//!   - the closing-fence check measures indentation in COLUMNS,
//!     container-relative (tab stops of 4), so `\t```` at EOF cannot
//!     false-close a block and a valid list-nested `  \t~~~` close does
//!     not raise a false unclosed-fence anomaly.
//!   - the hidden-content anomaly triggers are scanned INSIDE skipped
//!     code-bearing bodies, so `<code><template>…</template></code>`
//!     fails closed instead of vanishing.
//!   - self-closing detection requires the `/` to be tag syntax, not the
//!     tail of an unquoted attribute value.
//!   - the CDATA trigger is ASCII-case-insensitive.
//!   - a `</name` close match requires a real name boundary, so
//!     `</prefix>` does not close `<pre>`.
//!   - raw-HTML dominance requires an absolute net-markup byte floor
//!     (800) in addition to the 20% ratio, so an idiomatic badge-header
//!     README passes while markup-heavy docs still trip.

mod common;

use common::{assert_invariants, has_rule, rule_ids, run};
use unslop::Profile;

// Ordinary prose that dominates the byte count so the raw-HTML dominance
// branch of M005 is never what fires in the HTML cases below.
fn filler() -> String {
    "This paragraph is ordinary visible prose that carries the document. ".repeat(16)
}

// References to invisible/control codepoints fail closed -------------------

#[test]
fn invisible_codepoint_refs_fail_closed_as_anomaly() {
    let cases = [
        ("zwsp", "<div>Ordinary del&#8203;ve detail.</div>"),
        ("tab", "<div>Ordinary game&#9;changer detail.</div>"),
        ("c0 control", "<div>Ordinary del&#1;ve detail.</div>"),
        (
            "c1 control (no cp1252 remap)",
            "<div>Ordinary text&#151;following detail.</div>",
        ),
        // The classification is not HTML-specific: markdown prose decodes
        // numeric refs too, so the same evasion fails closed there.
        ("zwsp in markdown prose", "Ordinary del&#8203;ve detail."),
    ];
    for (label, src) in cases {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-M005"),
            "{label} must fail closed via M005: {:?}",
            rule_ids(&report)
        );
        assert!(
            !has_rule(&report, "SLOP-A001"),
            "{label}: the hidden word must not be fabricated into a match: {:?}",
            rule_ids(&report)
        );
        assert!(
            !has_rule(&report, "SLOP-M001"),
            "{label}: no decoded em-dash may appear (C1 stays unmapped): {:?}",
            rule_ids(&report)
        );
        assert_invariants(&text, &report);
    }
}

// CommonMark decode bounds kill the over-decode FPs ------------------------

#[test]
fn overlong_hex_ref_in_markdown_is_inert() {
    // 7 hex digits exceed the CommonMark bound (6): the renderer leaves this
    // literal, so nothing hides and nothing may fire — not the fabricated
    // "delve" (A001), not the ref's own `;` (M002), and, in markdown prose,
    // not an anomaly either.
    let text = format!("{f}\n\nOrdinary &#x0000064;elve detail.\n", f = filler());
    let report = run(&text, Profile::Doc);
    for rule in ["SLOP-A001", "SLOP-M002", "SLOP-M005"] {
        assert!(
            !has_rule(&report, rule),
            "overlong hex ref must be inert in markdown, fired {rule}: {:?}",
            rule_ids(&report)
        );
    }
    assert_invariants(&text, &report);
}

#[test]
fn overlong_ref_in_html_text_fails_closed_without_m002() {
    // A browser decodes `&#0000000169;` (©); the CommonMark grammar rejects
    // it. In HTML-derived text that divergence fails closed as M005, and the
    // undecoded ref's `;` must not read as prose punctuation (M002).
    let text = format!(
        "{f}\n\n<div>Copyright &#0000000169; 2026.</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "browser-decodable overlong ref in HTML text must fail closed: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-M002"),
        "the elided ref's semicolon must not fire M002: {:?}",
        rule_ids(&report)
    );
}

// Semicolonless refs: browser grammar in HTML text, inert in markdown

#[test]
fn semicolonless_ref_in_html_text_fails_closed() {
    // A browser renders `&#100elve` as "delve" (missing-semicolon recovery);
    // the decoder correctly refuses, so the divergence must fail closed
    // rather than silently pass the hidden word.
    let text = format!(
        "{f}\n\n<div>Ordinary &#100elve detail.</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "semicolonless ref in HTML text must fail closed: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "no fabricated word: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn semicolonless_ref_in_markdown_is_inert() {
    // CommonMark leaves `&#100elve` fully literal — the reader sees the
    // junk, nothing hides, nothing fires.
    let text = format!("{f}\n\nOrdinary &#100elve detail.\n", f = filler());
    let report = run(&text, Profile::Doc);
    for rule in ["SLOP-A001", "SLOP-M005"] {
        assert!(
            !has_rule(&report, rule),
            "semicolonless ref must be inert in markdown, fired {rule}: {:?}",
            rule_ids(&report)
        );
    }
}

// Guardrails — legitimate references still decode normally -----------------

#[test]
fn ordinary_refs_still_decode_and_do_not_anomaly_flag() {
    // Em-dash spellings decode (and correctly reach M001, the em-dash rule);
    // no anomaly.
    for dash in ["&#8212;", "&#x2014;"] {
        let text = format!("{f}\n\nRanges run 3{dash}5 wide here.\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-M001"),
            "{dash} must decode to an em-dash and reach M001: {:?}",
            rule_ids(&report)
        );
        assert!(
            !has_rule(&report, "SLOP-M005"),
            "{dash} is legitimate typography, not an anomaly: {:?}",
            rule_ids(&report)
        );
    }
    // © and é decode; space-separator refs fold to a plain space like
    // &nbsp;/&emsp; do (and a folded space still reaches two-word patterns).
    let text = format!(
        "{f}\n\nCopyright &#169; 2026 by the caf&#233; project.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "&#169;/&#233; decode without anomaly: {:?}",
        rule_ids(&report)
    );
    let text = format!("{f}\n\nA real game&#xA0;changer here.\n", f = filler());
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001") && !has_rule(&report, "SLOP-M005"),
        "an NBSP ref folds to a space and the two-word term still fires: {:?}",
        rule_ids(&report)
    );
}

// A tab-indented close line does not close the fence (fail-open) ------------

#[test]
fn tab_indented_fence_close_at_eof_fails_closed() {
    for fence in ["```", "~~~"] {
        let text = format!(
            "{f}\n\n{fence}\ncode\ndelve game-changer\n\t{fence}",
            f = filler()
        );
        let report = run(&text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-M005"),
            "a tab (4 columns) cannot close a {fence} fence; the block runs \
             to EOF unclosed and must fail closed: {:?}",
            rule_ids(&report)
        );
    }
}

// A valid list-nested close measured container-relative ---------------------

#[test]
fn list_nested_tab_close_is_a_real_close() {
    // Inside a `- ` item (content column 2) the closing line `  \t~~~` puts
    // the fence at column 4 — 2 columns past the container, a valid close.
    // Counting the item's own spaces as indent and reading the tab as a
    // run-breaking char would raise a false unclosed-fence M005.
    let text = format!("{f}\n\n- ~~~\n  code\n  \t~~~", f = filler());
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a container-relative 2-column close is valid: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn blockquote_fence_close_at_eof_still_closes() {
    let text = format!("{f}\n\n> ```\n> code\n> ```", f = filler());
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a quote-prefixed close at the opening column is valid: {:?}",
        rule_ids(&report)
    );
}

// Hidden-content triggers inside skipped code bodies ------------------------

#[test]
fn hidden_construct_nested_in_skipped_body_fails_closed() {
    let cases = [
        (
            "template inside code",
            "<div><code><template>delve game-changer</template></code></div>",
        ),
        ("cdata inside pre", "<pre>x<![CDATA[ delve ]]>y</pre>"),
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

// A `/` ending an unquoted attribute value is not self-closing --------------

#[test]
fn attribute_value_slash_is_not_self_closing() {
    let cases = [
        // `/` before a space: value of data-x, tag closed by the later `>`.
        "<div><script data-x=/ >ordinary script body</script></div>",
        // `/` as the tail of an unquoted value: src is "x.js/", not `<script/>`.
        "<div><script src=x.js/>ordinary body</script></div>",
    ];
    for src in cases {
        let text = format!("{f}\n\n{src}\n", f = filler());
        let report = run(&text, Profile::Doc);
        assert!(
            !has_rule(&report, "SLOP-M005"),
            "an attribute-value `/` must not read as self-closing for {src:?}: {:?}",
            rule_ids(&report)
        );
        assert!(
            !has_rule(&report, "SLOP-A001"),
            "the script body stays skip-excluded for {src:?}: {:?}",
            rule_ids(&report)
        );
    }
}

// Lowercase CDATA trips the same anomaly ------------------------------------

#[test]
fn lowercase_cdata_fails_closed_like_uppercase() {
    // Block-HTML context (`<div>` is a CommonMark type-6 tag, `<svg>` is
    // not): a case-sensitive trigger swallowed `<![cdata[ … ]]>` as tag
    // markup with no finding — invisible to a browser (bogus comment) AND
    // unscanned. It trips the same case-insensitive CDATA anomaly as
    // `<![CDATA[`.
    let text = format!("{f}\n\n<div><![cdata[ delve ]]></div>\n", f = filler());
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "case-folded CDATA must fail closed: {:?}",
        rule_ids(&report)
    );
}

// `</prefix>` does not close `<pre>` ----------------------------------------

#[test]
fn close_tag_requires_name_boundary() {
    // A boundary-less match reads `</pre` inside `</prefix>`, ending the skip
    // early and leaking the rest of the code body ("delve game-changer") to
    // the prose scan. With the boundary rule the body runs to the real
    // `</pre>`.
    let text = format!(
        "{f}\n\n<pre>x </prefix>delve game-changer</pre>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "the pre body must stay excluded from the prose scan: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a real close exists; no anomaly: {:?}",
        rule_ids(&report)
    );
}

// M005 dominance — absolute floor plus ratio -------------------------------

#[test]
fn badge_header_readme_clears_dominance_floor() {
    // An idiomatic centered badge header: ~700 bytes of pure markup in a
    // ~1.5 KB README — over the 20% ratio, under the 800-byte floor.
    let text = "<div align=\"center\">\n  <img src=\"assets/logo.png\" alt=\"logo\" width=\"120\">\n  <p>\n    <a href=\"https://crates.io/crates/demo-tool\"><img src=\"https://img.shields.io/crates/v/demo-tool.svg\" alt=\"crates.io\"></a>\n    <a href=\"https://docs.rs/demo-tool\"><img src=\"https://docs.rs/demo-tool/badge.svg\" alt=\"docs.rs\"></a>\n    <a href=\"https://github.com/org/demo-tool/actions\"><img src=\"https://img.shields.io/badge/ci-passing-green.svg\" alt=\"ci\"></a>\n    <a href=\"LICENSE\"><img src=\"https://img.shields.io/badge/license-MIT-blue.svg\" alt=\"license\"></a>\n  </p>\n</div>\n\n# demo-tool\n\nReads records from an input file and writes them back out in a stable order.\nThe command line takes a path and an optional format flag.\n\n## Usage\n\nInstall the binary with cargo, point it at a file, and read the exit code.\nA zero exit means every record parsed; anything else names the first bad line.\n\n## Notes\n\nThe format is documented in the spec file next to this readme, and the parser\naccepts either line ending. Large files stream in constant memory, and the\nrecord order in the output always matches the order of the input exactly.\nErrors go to standard error with the line number and the offending column.\nThe exit codes are listed in the manual page installed beside the binary.\n";
    assert!(
        text.len() > 1200 && text.len() < 2000,
        "fixture stays README-sized"
    );
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "an idiomatic badge-header README must not trip dominance: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn small_tag_soup_below_floor_is_accepted() {
    // ~400 bytes of pure markup: over the ratio, under the floor. An
    // accepted edge — dominance is a coverage signal; every hiding
    // construct is independently fail-closed.
    let text = "<div class=\"a\" data-x=\"1\" id=\"root\" role=\"main\" style=\"display:flex\">\n<span class=\"i\" data-k=\"v\"></span><span class=\"i\" data-k=\"w\"></span>\n<div class=\"b\" style=\"color:red;padding:4px;margin:2px;border:1px solid black\"></div>\n</div>\n";
    assert!(text.len() < 800, "fixture stays under the floor");
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "sub-floor tag soup no longer trips dominance: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn hidden_html_heavy_doc_still_trips_dominance() {
    // Comments are net markup (never visible text), so a comment-stuffed doc
    // clears the floor and the ratio and still fails.
    let comment = "<!-- a long hidden annotation block that a reader never sees at all -->\n";
    let text = format!(
        "# notes\n\nA short visible paragraph that the reader does see.\n\n{}",
        comment.repeat(14)
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "a hidden-HTML-heavy doc must still trip dominance: {:?}",
        rule_ids(&report)
    );
}

// Literal invisible/default-ignorable chars cannot hide a word.
// The execution-confirmed shapes: `del\u{00AD}ve` (soft hyphen),
// `del\u{200E}ve` (LRM), `del\u{202A}ve` (LRE) exited CLEAN when the
// literal-path checks keyed on the 5-char ZERO_WIDTH set. The norm view
// removes the FULL Unicode Default_Ignorable_Code_Point set (including
// non-Cf members like CGJ U+034F and variation selectors), so the word
// normalizes to its plain spelling and the lexicon fires directly.

#[test]
fn literal_default_ignorables_cannot_hide_lexicon_words() {
    let chars = [
        ("soft hyphen U+00AD", '\u{00AD}'),
        ("LRM U+200E", '\u{200E}'),
        ("LRE U+202A", '\u{202A}'),
        ("CGJ U+034F (non-Cf default-ignorable)", '\u{034F}'),
        ("halfwidth hangul filler U+FFA0", '\u{FFA0}'),
        ("VS15 U+FE0E", '\u{FE0E}'),
    ];
    for (label, ch) in chars {
        for (ctx, src) in [
            ("markdown", format!("Ordinary del{ch}ve detail.")),
            ("html", format!("<div>Ordinary del{ch}ve detail.</div>")),
        ] {
            let text = format!("{f}\n\n{src}\n", f = filler());
            let report = run(&text, Profile::Doc);
            assert!(
                has_rule(&report, "SLOP-A001"),
                "{label} in {ctx} must normalize away and expose the word: {:?}",
                rule_ids(&report)
            );
            assert_invariants(&text, &report);
        }
    }
}

#[test]
fn numeric_refs_to_default_ignorables_still_anomaly() {
    // The REF spelling of the same codepoints keeps the same answer:
    // fail-closed M005, never decoded, no fabricated word — in markdown AND
    // HTML contexts. `&#847;` (CGJ), `&#xFFA0;`, and `&#xFE0E;` cover the
    // non-Cf default-ignorables, which once decoded as
    // ordinary chars. `&#x2064;` (INVISIBLE PLUS, Cf) is the control proving
    // the Cf suppression is untouched.
    for r in [
        "&#173;", "&#x200E;", "&#847;", "&#xFFA0;", "&#xFE0E;", "&#x2064;",
    ] {
        for (ctx, src) in [
            ("markdown", format!("Ordinary del{r}ve detail.")),
            ("html", format!("<div>Ordinary del{r}ve detail.</div>")),
        ] {
            let text = format!("{f}\n\n{src}\n", f = filler());
            let report = run(&text, Profile::Doc);
            assert!(
                has_rule(&report, "SLOP-M005"),
                "{r} in {ctx} must stay a fail-closed anomaly: {:?}",
                rule_ids(&report)
            );
            assert!(
                !has_rule(&report, "SLOP-A001"),
                "{r} in {ctx} must not decode into a match: {:?}",
                rule_ids(&report)
            );
        }
    }
}

#[test]
fn soft_hyphen_hyphenation_hint_stays_clean() {
    // The legitimate use of U+00AD: a hyphenation hint inside a long
    // non-lexicon word. Removal must not create a spurious finding — no
    // fabricated lexicon hit, no anomaly, no M004 (whose pattern class
    // deliberately stays the original zero-width set).
    let text = format!(
        "{f}\n\nThe in\u{00AD}ternation\u{00AD}alization effort continues on schedule.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    for rule in ["SLOP-A001", "SLOP-M004", "SLOP-M005"] {
        assert!(
            !has_rule(&report, rule),
            "a hyphenation hint must stay clean, fired {rule}: {:?}",
            rule_ids(&report)
        );
    }
    assert_invariants(&text, &report);
}
