//! Regression coverage for fence-length enforcement, wrapped HTML blocks,
//! the visible-text tokenizer, and trigger fidelity:
//!
//!   - the fence-open-length check holds: a shorter fence run does not
//!     close a longer-opened block, so a 3-backtick line cannot swallow a
//!     4-backtick-opened block's slop tail as code (an execution-confirmed
//!     fail-open shape).
//!   - consecutive block-HTML events separated only by a blockquote/list
//!     continuation gap are bridged into one logical block, so a wrapped
//!     multi-line comment reaches `html_comments` (Y001) without a spurious
//!     unclosed anomaly and without leaking a comment tail into prose, while
//!     genuinely separate HTML blocks still flush separately.
//!   - the HTML visible-text tokenizer is quote-aware, applies the HTML5
//!     `<`-is-text rule, preserves element-boundary spaces, inserts hard
//!     boundaries between block elements, and skips pre/code/kbd/samp bodies;
//!     every script/style body is inspected and an unclosed one fails closed.
//!   - a finding whose reported span does not render back to the matched
//!     trigger becomes an instrumentation error, never a wrong finding.

mod common;

use common::{has_rule, rule_ids, run, snippet, violations};
use unslop::Profile;

// Ordinary prose that dominates the byte count so the raw-HTML dominance branch
// of M005 cannot be what fires in the HTML cases below.
fn filler() -> String {
    "This paragraph is ordinary visible prose that carries the document. ".repeat(15)
}

// Fence-open length ------------------------------------------------------------------------

#[test]
fn short_fence_close_at_eof_fails_closed() {
    // The exact fail-open input: a 4-backtick fence, `code`, a 3-backtick
    // line, a slop line, then a 3-backtick line at EOF. Without the open-length
    // check the trailing 3-backtick line "closed" the 4-backtick block
    // (run 3 >= 3) and the slop was swallowed as code — exit 0, no finding.
    // `run >= open_len.max(3)` keeps the block unclosed and M005
    // fires.
    let text = "This report describes a build failure on the current release branch.\n\
         Steps to reproduce are listed below with the observed output.\n\n\
         ````\ncode\n```\ndelve game-changer\n```";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "a short fence close at EOF must fail closed via M005: {:?}",
        rule_ids(&report)
    );
    assert!(
        !violations(&report).is_empty(),
        "the swallowed slop tail must surface as a violation: {:?}",
        rule_ids(&report)
    );
}

// Wrapped HTML blocks ------------------------------------------------------------------------

#[test]
fn blockquoted_multiline_comment_reaches_html_comments() {
    // pulldown emits the two lines of a blockquoted comment as separate Html
    // events with a "> " continuation gap. Bridged, the comment closes and its
    // content reaches doc.html_comments: Y001 fires (Readme), and neither the
    // spurious unclosed-comment M005 nor a prose scan of the comment tail (A001)
    // appears.
    let text = format!(
        "{f}\n\n> <!-- delve into this rich tapestry, a game-changer\n> and let us not forget the synergy -->\n> Quoted text.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-Y001"),
        "blockquoted multi-line comment content must reach html_comments (Y001): {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a CLOSED blockquoted comment must not raise the unclosed anomaly: {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "comment content must not be scanned as visible prose (no A001 from the tail): {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn blockquoted_div_visible_text_is_scanned_and_blocks() {
    // The first line of a blockquoted <div> must be scanned: the bridged block
    // carries the visible text, so the banned words fire and the document has a
    // violation. (The "> " continuation gap is interior visible text, harmless.)
    let text = format!(
        "{f}\n\n> <div>Let us delve into the rich tapestry here\n> and more visible text</div>\n\nEnd.\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "slop in the FIRST line of a blockquoted div must fire A001: {:?}",
        rule_ids(&report)
    );
    assert!(
        !violations(&report).is_empty(),
        "blockquoted div slop must produce a violation: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn list_indented_multiline_comment_bridges() {
    // A list-indented comment's continuation gap is "  " (indent, no marker);
    // it must bridge the same way so the comment reaches html_comments.
    let text = format!(
        "{f}\n\n- <!-- hidden note spanning\n  a second indented line -->\n- second item\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-Y001"),
        "list-indented multi-line comment must reach html_comments (Y001): {:?}",
        rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a closed list-indented comment must not raise the unclosed anomaly: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn separate_html_blocks_still_flush_separately() {
    // Two divs separated by a blank line are distinct blocks (an intervening
    // event flushes the first). Each is scanned in isolation, so slop in the
    // SECOND still fires — proof the join did not drop the first fragment.
    let text = format!(
        "{f}\n\n<div>ordinary first block</div>\n\n<div>we delve into it</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "slop in a SEPARATE second HTML block must still fire: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

// Script/style bodies -------------------------------------------------

#[test]
fn every_script_body_is_inspected() {
    // Two <script> blocks in one HTML region: the OLD code found only the first.
    // Both bodies are render-dropped text and must each reach Y001.
    let text = format!(
        "{f}\n\n<div>\n<script>var a = 1;</script>\n<p>text</p>\n<script>hidden second body here</script>\n</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let y001: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-Y001")
        .map(snippet)
        .collect();
    assert!(
        y001.iter().any(|s| s.contains("var a = 1;")),
        "first script body must reach Y001: {y001:?}"
    );
    assert!(
        y001.iter().any(|s| s.contains("hidden second body here")),
        "SECOND script body must also reach Y001: {y001:?}"
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn unclosed_script_fails_closed() {
    // A <script> with no </script> runs to end-of-block/EOF and is swallowed by
    // every scanner. In parity with the unclosed comment it must fire M005.
    let text = format!(
        "{f}\n\n<script>\nthis body never closes and hides delve tapestry from every scan\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "an unclosed <script> must fail closed via M005: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn script_body_span_is_quote_aware() {
    // The mirror bug: a `>` inside a quoted attribute value must not end the
    // start tag, so the reported body is the body — not attribute bytes.
    let text = format!(
        "{f}\n\n<script title=\">x\">body content here</script>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let body = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-Y001")
        .map(snippet)
        .expect("script body must reach Y001");
    assert_eq!(
        body, "body content here",
        "quote-aware tag end: no attribute bytes may leak into the body span"
    );
}

// Visible-text tokenizer ----------------------------------------------

#[test]
fn quote_aware_tag_end_does_not_leak_attributes() {
    // A `>` inside a quoted attribute value must not end the tag, so the
    // attribute text is not dumped into prose (attribute-only content is out of
    // scope): no banned word from inside `title="… delve …"`.
    let text = format!(
        "{f}\n\n<div title=\"a tapestry to delve into\">Plain visible sentence.</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "attribute-value slop must not surface (attributes out of scope): {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn html5_stray_lt_is_visible_text() {
    // `< delve` — a `<` not followed by a letter/`/`/`!`/`?` is literal text,
    // not a tag start, so the words after it render and must fire.
    let text = format!(
        "{f}\n\n<div>note: x < delve into the rich tapestry > done</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let a001: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == "SLOP-A001")
        .map(snippet)
        .collect();
    assert!(
        a001.iter().any(|s| s == "delve"),
        "text after a literal `<` must be scanned (delve): {a001:?}"
    );
    assert!(
        a001.iter().any(|s| s == "tapestry"),
        "text before a literal `>` must be scanned (tapestry): {a001:?}"
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn element_boundary_space_is_preserved() {
    // `game <i>changer</i>` renders "game changer"; the element-boundary space
    // must be kept so the multi-word cliché matches.
    let text = format!(
        "{f}\n\n<div>a real game <i>changer</i> today</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "space across inline markup must be kept so `game changer` matches: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn nested_inline_stays_word_bounded() {
    // `ordinary <span>delve</span> detail` must read "ordinary delve detail"
    // (spaces kept), so `delve` fires word-bounded and does not fuse its
    // neighbours into "ordinarydelvedetail".
    let text = format!(
        "{f}\n\n<div>ordinary <span>delve</span> detail</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let hit = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .expect("delve inside a span must fire A001");
    assert_eq!(snippet(hit), "delve", "span must land exactly on the word");
    common::assert_invariants(&text, &report);
}

#[test]
fn block_boundary_does_not_fuse_across_elements() {
    // `<div>game&nbsp;</div><div>changer</div>` must NOT produce a cross-block
    // "game changer": the block boundary is a hard separator.
    let text = format!(
        "{f}\n\n<div>game&nbsp;</div><div>changer</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "text must not fuse across a block-element boundary: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn pre_code_body_is_not_prose() {
    // Code inside HTML is not prose: a `<details><pre>` code sample with SQL
    // caps must not raise the insistence-formatting E002 the visible-text
    // extraction used to fire on it.
    let text = format!(
        "{f}\n\n<details><summary>example</summary><pre>SELECT * FROM t WHERE x IS NOT NULL</pre></details>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-E002"),
        "a <pre> code body must be skipped, not scanned as prose: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

// Trigger fidelity ------------------------------------------------------------------------

#[test]
fn softbreak_crossing_finding_is_not_false_tripped() {
    // A contrast cliché that spans a soft line break: the norm folds "\n" to a
    // space, so the source slice differs from the trigger. The fidelity check
    // must accept it (fold + containment), not convert the finding to an
    // instrumentation error; a false trip would fail `analyze` and panic `run`.
    let text = "This is not just a parser,\nbut a comprehensive framework for everything.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-C001") || !report.findings.is_empty(),
        "the softbreak-crossing finding must survive the fidelity check: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn inline_code_split_word_does_not_fire() {
    // An INVERSION of an earlier premise, which asserted the OPPOSITE: that
    // `de`x`lve` fuses to "delve" in the norm view, fires SLOP-A001, and
    // survives fidelity via the norm-text reconstruction. That premise was a
    // false positive — the rendered text is "de x lve" (the code content
    // visibly interrupts the word), so no reader ever sees "delve". The
    // barrier puts U+FFFD in the norm view at the inline-code gap; the word
    // never assembles and nothing fires — and the fidelity check no longer
    // has a fused span to certify.
    let text = "Intro sentence for length and context here.\n\nWe de`x`lve into the topic.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "inline-code-split fragments must not assemble a word: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}
