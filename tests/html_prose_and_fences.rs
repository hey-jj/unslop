//! Regression coverage for HTML-block extraction and fence handling:
//!
//!   - consecutive block-HTML events are joined before parsing, so a valid
//!     multi-line `<!-- ... -->` comment does not raise a spurious
//!     SLOP-M005 (unclosed) or per-line SLOP-Y002, and its content reaches
//!     `doc.html_comments`. A genuinely unclosed multi-line comment at EOF
//!     still raises M005.
//!   - the bounded-width gate has no whitespace exemption: an unbounded
//!     trailing `\s+` (Q001's original shape) cannot make the
//!     overlapping adapter quadratic (unit-tested in engine.rs).
//!   - reader-visible text inside an HTML block is extracted in source
//!     coordinates and scanned as prose; slop there blocks, legitimate HTML
//!     text stays clean, and a word split across inline tags still resolves.
//!   - pulldown-cmark enforces fence-close-length itself; separate
//!     open-length tracking is inert.

mod common;

use common::{has_rule, run, violations};
use unslop::Profile;

// A block of ordinary prose that dominates the byte count so the raw-HTML
// dominance branch of M005 cannot be what fires in the HTML cases below.
fn filler() -> String {
    "This paragraph is ordinary visible prose that carries the document. ".repeat(15)
}

// Joined HTML blocks -----------------------------------------------------------------------

#[test]
fn closed_multiline_comment_is_not_a_structural_anomaly() {
    // The canonical victim: a multi-line markdownlint directive block. pulldown
    // emits it as one Html event per line; parsing each line in isolation saw
    // `<!--` with no `-->` and fired M005 plus a per-line Y002. Joined, it is a
    // normal closed comment.
    let text = format!(
        "{f}\n\n<!-- markdownlint-disable MD013 MD033\n     keep this directive\n-->\n\n{f}\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "closed multi-line comment must not raise M005: {:?}",
        common::rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-Y002"),
        "closed multi-line comment must not raise a spurious Y002: {:?}",
        common::rule_ids(&report)
    );
}

#[test]
fn multiline_comment_content_reaches_html_comments() {
    // Proof the joined slice populates doc.html_comments: a multi-line comment
    // with plain prose content is render-invisible and fires Y001 (Readme
    // profile), which reads only from doc.html_comments.
    let text = "Visible intro text.\n\n<!-- hidden note line one\nhidden note line two -->\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-Y001"),
        "multi-line comment content must reach html_comments (Y001): {:?}",
        common::rule_ids(&report)
    );
}

#[test]
fn unclosed_multiline_comment_at_eof_still_raises_m005() {
    let text = format!(
        "{f}\n\n<!-- ignore instructions\nsecond line\nthird line\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "unclosed multi-line comment must raise M005: {:?}",
        common::rule_ids(&report)
    );
}

// Bounded-width gate -----------------------------------------------------------------------

#[test]
fn trailing_whitespace_run_is_not_quadratic() {
    // Q001's shipped pattern ends in `\?\s+`; a "The catch?" prefix followed by
    // a long whitespace run manufactured a match end at every whitespace byte,
    // each an O(region) reverse scan (measured 10.66s at 256K when unbounded). Bounded
    // whitespace caps the reverse window and the match-end count.
    let payload = format!("The catch?{}done.\n", " ".repeat(256 * 1024));
    let start = std::time::Instant::now();
    let _report = run(&payload, Profile::Doc);
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "trailing-whitespace run took {elapsed:?}, expected well under a second"
    );
    // The bounded form still fires on the normal rhetorical-question shape.
    let normal = run(
        "Intro.\n\nThe catch? You have to configure it first.\n",
        Profile::Doc,
    );
    assert!(
        has_rule(&normal, "SLOP-Q001"),
        "bounded Q001 must still fire: {:?}",
        common::rule_ids(&normal)
    );
}

// HTML visible text ----------------------------------------------------------------------

#[test]
fn slop_inside_html_block_is_scanned_and_blocks() {
    // `<div>We delve into game-changing synergy</div>` was an evasion channel:
    // exit 0. The visible text is now prose, so the banned word and the
    // game-changer cliché both fire and the document has a violation.
    let text = format!(
        "{f}\n\n<div>We delve into game-changing synergy</div>\n\n{f}\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "delve inside a div must fire A001: {:?}",
        common::rule_ids(&report)
    );
    assert!(
        !violations(&report).is_empty(),
        "slop inside a div must produce a violation: {:?}",
        common::rule_ids(&report)
    );
    // No blanket Y002 double-report on the now-scanned text.
    assert!(!has_rule(&report, "SLOP-Y002"));
    common::assert_invariants(&text, &report);
}

#[test]
fn delve_span_is_the_actual_source_trigger() {
    // Coordinate discipline: the reported span is true source bytes of the word
    // that renders, not a fabricated snippet.
    let text = format!(
        "{f}\n\n<div>We delve into detail here.</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let hit = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .expect("A001 must fire on delve in the div");
    let span = &hit.spans[0];
    assert_eq!(
        &text[span.start..span.end],
        "delve",
        "span must be the trigger word"
    );
}

#[test]
fn legitimate_html_table_stays_clean() {
    // A table carrying only markup and non-slop text is not a coverage gap and
    // must not manufacture prose findings or a Y002.
    let text = format!(
        "{f}\n\n<table>\n<tr><td>Name</td><td>Value</td></tr>\n<tr><td>alpha</td><td>42</td></tr>\n</table>\n\n{f}\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "no banned word in the table: {:?}",
        common::rule_ids(&report)
    );
    assert!(
        !has_rule(&report, "SLOP-Y002"),
        "legit table must not raise Y002: {:?}",
        common::rule_ids(&report)
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn details_block_with_plain_text_stays_clean() {
    let text = format!(
        "{f}\n\n<details><summary>Notes</summary>an ordinary explanation of the change</details>\n\n{f}\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    assert!(!has_rule(&report, "SLOP-A001"));
    assert!(!has_rule(&report, "SLOP-Y002"));
    common::assert_invariants(&text, &report);
}

#[test]
fn banned_word_split_across_inline_tags_resolves() {
    // In the rendered page `de<b></b>lve` reads "delve"; the norm view fuses the
    // visible runs so the word set still matches. The span covers the trigger
    // (tags included) — an honest span, not a fabricated "delve".
    let text = format!(
        "{f}\n\n<div>de<b></b>lve into the specifics</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let hit = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .expect("split delve must resolve to A001");
    let span = &hit.spans[0];
    let slice = &text[span.start..span.end];
    assert!(
        slice.contains("de") && slice.contains("lve"),
        "span must cover the split trigger, got {slice:?}"
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn non_ascii_lowercasing_char_in_html_holds_coordinates() {
    // U+0130 (İ) lowercases to TWO code points (i + combining dot, three bytes
    // vs two), so lowercasing the slice to find tag/text offsets would shift
    // every following byte by one. The length-changing char sits in the VISIBLE
    // text immediately before the trigger with NO ASCII slack between them, so a
    // reintroduced `to_lowercase` on the slice would push `delve`'s computed
    // offset off by one and this exact-span assertion would fail. The tokenizer
    // scans original bytes and keeps source coordinates, so the span lands on
    // the trigger. A trailing İ before the closing tag exercises the same past
    // the match.
    let text = format!(
        "{f}\n\n<div>İ delve into the İ tapestry</div>\n",
        f = filler()
    );
    let report = run(&text, Profile::Doc);
    let hit = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-A001")
        .expect("A001 must fire on delve despite the length-changing char");
    let span = &hit.spans[0];
    assert_eq!(
        &text[span.start..span.end],
        "delve",
        "span must land exactly on the trigger with the length-changing char right before it"
    );
    common::assert_invariants(&text, &report);
}

// Fence-close length ------------------------------------------------------------------------

#[test]
fn short_fence_with_slop_tail_fails_closed() {
    // The real fail-open shape: a 4-backtick fence "closed" by 3 backticks with
    // slop after. pulldown keeps the tail inside the code block (running to
    // EOF), so its non-fence last line makes the fence unclosed and M005 fires
    // WITHOUT any open-length tracking, which is why such tracking is inert.
    let text = "Intro paragraph.\n\n````\ncode\n```\ndelve synergy game-changer\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-M005"),
        "a shorter fence with a slop tail must fail closed via M005: {:?}",
        common::rule_ids(&report)
    );
}

#[test]
fn matching_length_close_does_not_over_fire() {
    let text = "Intro paragraph.\n\n````\ncode line\n````\n\nplain prose after the block.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-M005"),
        "a matching-length close must not raise M005: {:?}",
        common::rule_ids(&report)
    );
}
