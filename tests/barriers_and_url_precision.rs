//! Engine-precision coverage: Unicode word-boundary post-filters on
//! the ASCII DFA prefilter, the inline-code/autolink norm barrier,
//! and link-URL region precision for reference definitions and repeated
//! inline URLs.

mod common;

use common::{has_rule, rule_ids, run};
use unslop::Profile;

// Norm barriers ---------------------------------------------------------------

#[test]
fn word_split_by_inline_code_does_not_fire() {
    // Without the barrier the norm view concatenates across the excluded
    // inline-code gap, assembling "delve" from "del" + "ve" and firing
    // SLOP-A001 as a BLOCKING
    // violation on a word the reader never sees (the rendered text is
    // "del x ve", with x in code font). The barrier interposes U+FFFD so the
    // flanking runs can never fuse.
    let text = "Intro sentence for length and context here.\n\nWe del`x`ve into the topic.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "inline-code-split fragments must not assemble a lexicon word: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn genuine_word_still_fires() {
    let text = "Intro sentence for length and context here.\n\nWe delve into the topic.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "the barrier must not affect a genuine word: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn inline_code_does_not_manufacture_a_phrase() {
    // The barrier must be U+FFFD, not a space: a space between "game" and
    // "changer" would assemble the two-word phrase "game changer" that the
    // reader never sees adjacently (the code content renders between them).
    let text = "Intro sentence for length and context here.\n\nA game`x`changer for the team.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "inline code between words must not assemble a phrase: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);

    // Control: the genuine phrase still fires.
    let text = "Intro sentence for length and context here.\n\nA game changer for the team.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "the genuine phrase must still fire: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn autolink_gap_is_a_barrier_too() {
    // An autolink is an excluded inline region whose URL text renders VISIBLY
    // between the flanking runs — same class as inline code: "del" + "ve"
    // must not fuse across it.
    let text =
        "Intro sentence for length and context here.\n\nWe del<https://example.com/a>ve into it.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "autolink-split fragments must not assemble a lexicon word: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

// Unicode boundary post-filters ------------------------------------------------

#[test]
fn nonascii_word_prefix_blocks_q001_boundary() {
    // é is xid_continue, so "éwhy" is ONE word and "why" is not at a real
    // word boundary. The ASCII `(?-u:\b)` prefilter sees a boundary inside
    // the token; the Unicode post-filter must reject the candidate.
    let text = "Intro sentence for context.\n\néwhy does this matter?\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-Q001"),
        "no real word boundary inside a non-ASCII token: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);

    // Control: at a genuine boundary the rule still fires.
    let text = "Intro sentence for context.\n\nwhy does this matter?\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-Q001"),
        "the genuine rhetorical question must still fire: {:?}",
        rule_ids(&report)
    );
}

#[test]
fn nonascii_double_hyphen_fires_m001() {
    // The `(?<=\w)--(?=\w)` lookaround was prefiltered with ASCII `\w`, so a
    // double hyphen between non-ASCII word characters went unflagged. The
    // widened prefilter plus the is_xid_continue post-filter closes it.
    for text in [
        "A café--style approach to the layout.\n",
        "The 变量--值 mapping in prose.\n",
        "The build--system integration notes.\n", // ASCII control, unchanged
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-M001"),
            "double hyphen between word chars must fire in {text:?}: {:?}",
            rule_ids(&report)
        );
        common::assert_invariants(text, &report);
    }
}

#[test]
fn word_set_unicode_boundary_unchanged() {
    // The word-set side already validates boundaries with is_xid_continue:
    // "édelve" is one token and must not fire A001. Guard against regression.
    let text = "Intro sentence for length and context here.\n\nThe édelve token appears.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "édelve is one word; A001 must not fire: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

// Link-URL region precision ---------------------------------------------------

#[test]
fn refdef_label_is_not_a_url_region() {
    // The whole refdef span (label included) was treated as a URL region, so
    // a label that happens to spell a tracking parameter fired SLOP-P004 even
    // though the actual destination is clean. Only the destination substring
    // is URL text.
    let text = "See [clean][utm_source=chatgpt] for details.\n\n\
                [utm_source=chatgpt]: https://example.com/docs\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-P004"),
        "a refdef LABEL is not URL text; the destination is clean: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn refdef_destination_still_fires() {
    let text = "See [docs][ref] for details.\n\n\
                [ref]: https://example.com/?utm_source=chatgpt\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-P004"),
        "a tracking param in the refdef DESTINATION must still fire: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn repeated_inline_url_reports_the_destination() {
    // The destination and the title carry the same URL; `rfind` over the
    // whole link span picked the TITLE occurrence. The destination is the
    // first occurrence after the `](` delimiter.
    let url = "https://example.com/?utm_source=chatgpt";
    let text = format!("[link]({url} \"{url}\")\n");
    let report = run(&text, Profile::Doc);
    let dest_at = text.find(url).expect("destination occurrence");
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-P004")
        .expect("P004 must fire on the destination");
    assert!(
        f.spans[0].start >= dest_at && f.spans[0].end <= dest_at + url.len(),
        "P004 must report the DESTINATION occurrence at {dest_at}, got {}..{}",
        f.spans[0].start,
        f.spans[0].end
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn footnote_reference_gap_is_a_barrier() {
    // A footnote marker is RENDERED (the reader sees "del¹ve", never
    // "delve"), so a word split by one is the same FP class as the
    // inline-code gap: the flanking runs must not fuse.
    let text = "Intro sentence for length and context here.\n\n\
                We del[^1]ve into it.\n\n[^1]: A note.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "footnote-split fragments must not assemble a lexicon word: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);

    // Control: the common shape — a marker AFTER a completed word — still
    // fires, because U+FFFD is non-xid and the word boundary holds.
    let text = "Intro sentence for length and context here.\n\n\
                We delve[^1] into it.\n\n[^1]: A note.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "a word followed by a footnote marker must still fire: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

// Decoded destinations and void-tag barriers ----------------------------------

#[test]
fn render_affecting_void_html_is_a_barrier() {
    // A RENDER-AFFECTING void tag puts a visible break or object between the
    // flanking runs — the reader never sees the text fused — so it is the
    // same barrier class as inline code (narrowed to exactly this set).
    for text in [
        "Intro sentence for length and context here.\n\nWe del<br>ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<img alt=\"m\" src=\"x.png\">ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<hr>ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<input>ve into it.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            !has_rule(&report, "SLOP-A001"),
            "render-affecting void tag must not let fragments assemble in {text:?}: {:?}",
            rule_ids(&report)
        );
        common::assert_invariants(text, &report);
    }
    // GUARDRAIL: NON-rendering void tags leave the text visually
    // fused — `del<wbr>ve` reads "delve" — so barriering them would be a
    // hide-a-word evasion channel. They fuse and FIRE, like formatting tags.
    for text in [
        "Intro sentence for length and context here.\n\nWe del<meta>ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<link>ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<wbr>ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<b></b>ve into it.\n",
        "Intro sentence for length and context here.\n\nWe del<span></span>ve into it.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-A001"),
            "non-rendering-tag fusion is render-faithful and must fire in {text:?}: {:?}",
            rule_ids(&report)
        );
    }
}

#[test]
fn image_alt_has_barriers_but_stays_scanned() {
    // The image is a replaced object: alt text must not fuse with flanking
    // prose ("![del](image.png)ve" must not assemble "delve")...
    let text = "Intro sentence for length and context here.\n\nSee ![del](image.png)ve here.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-A001"),
        "alt text must not fuse with flanking prose: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);

    // ...but the alt text itself is visible-fallback prose and stays scanned.
    let text =
        "Intro sentence for length and context here.\n\nSee ![delve into](image.png) here.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-A001"),
        "alt text is still scanned prose: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn p004_matches_the_decoded_destination() {
    // The raw spelling hides the tracking param behind a backslash escape or
    // a character reference the parser decodes; the reader's URL carries
    // `utm_source=chatgpt` either way. Without the decoded pass both were
    // silent FNs (the destination was not even located, so the region went
    // unscanned).
    for text in [
        "See [docs](https://e/?utm\\_source=chatgpt) here.\n",
        "See [docs](https://e/?utm&#95;source=chatgpt) here.\n",
        // Refdef destination with an escape.
        "See [docs][r] here.\n\n[r]: https://e/?utm\\_source=chatgpt\n",
        // Control: the raw literal spelling still fires.
        "See [docs](https://e/?utm_source=chatgpt) here.\n",
        // Autolink with the LITERAL token still fires (raw matching).
        "See <https://e/?utm_source=chatgpt> here.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            has_rule(&report, "SLOP-P004"),
            "decoded destination must fire P004 in {text:?}: {:?}",
            rule_ids(&report)
        );
        common::assert_invariants(text, &report);
    }
    // Clean decoded destination: nothing.
    let text = "See [docs](https://e/?q=to\\_do) here.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-P004"),
        "a clean escaped destination must not fire: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);

    // AUTOLINKS are matched RAW: CommonMark does not decode
    // references inside an autolink URI — the renderer amp-escapes it, so
    // the browser href carries the literal `&#95;`/`&lowbar;` bytes, never a
    // decoded tracking token. Firing here would be a false positive.
    for text in [
        "See <https://e/?utm&#95;source=chatgpt> here.\n",
        "See <https://e/?utm&lowbar;source=chatgpt> here.\n",
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            !has_rule(&report, "SLOP-P004"),
            "an autolink reference stays literal in the href; no P004 in {text:?}: {:?}",
            rule_ids(&report)
        );
        common::assert_invariants(text, &report);
    }
}

#[test]
fn code_span_delimiter_label_does_not_abort() {
    // A label carrying a code span that spells `](…)` mislocates the
    // destination lookup onto the code-span URL. Requirement: the report
    // must not abort (this shape once was an exit-30 instrumentation
    // error — `run` would panic); the decoded-destination fallback still
    // fires P004, with the span on the label occurrence (accepted
    // span-precision residual on manufactured input).
    let text = "See [label `](https://clean)` tail](https://e/?utm\\_source=chatgpt) here.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-P004"),
        "the real destination's tracking param must still surface: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}

#[test]
fn escaped_bracket_label_does_not_claim_the_delimiter() {
    // Opportunistic close of the escaped-`](` edge: a label containing a
    // literal `\](` plus the same URL repeated must report the DESTINATION
    // occurrence, not the label one.
    let url = "https://e/?utm_source=chatgpt";
    let text = format!("See [a\\]({url}]({url}) here.\n");
    let report = run(&text, Profile::Doc);
    let dest_at = text.rfind(url).expect("destination occurrence");
    let f = report
        .findings
        .iter()
        .find(|f| f.rule_id == "SLOP-P004")
        .expect("P004 must fire on the destination");
    assert!(
        f.spans[0].start >= dest_at,
        "P004 must report the destination at {dest_at}, got {}..{}",
        f.spans[0].start,
        f.spans[0].end
    );
    common::assert_invariants(&text, &report);
}

#[test]
fn entities_outside_the_enumerated_table_fire() {
    // pulldown decodes the FULL HTML5 entity table in link destinations;
    // `&lowbar;`/`&period;` are outside the crate's enumerated render_key
    // table. Routed through render_key the whole report would ABORT as an
    // instrumentation error (exit 30) instead of firing — `run` panics on
    // that, so these assertions also prove no exit-30. Trigger fidelity
    // verifies against the parser-decoded destination text carried on the
    // hit.
    for (text, rule) in [
        (
            "See [d](https://e/?utm&lowbar;source=chatgpt) here.\n",
            "SLOP-P004",
        ),
        (
            "See [d][r] here.\n\n[r]: https://e/?utm&lowbar;source=chatgpt\n",
            "SLOP-P004",
        ),
        (
            "See [f](https://files&period;oaiusercontent.com/x) here.\n",
            "SLOP-P002",
        ),
    ] {
        let report = run(text, Profile::Doc);
        assert!(
            has_rule(&report, rule),
            "{rule} must fire on the decoded destination in {text:?}: {:?}",
            rule_ids(&report)
        );
        common::assert_invariants(text, &report);
    }
}

#[test]
fn autolink_p002_literal_fires_entity_does_not() {
    // Same raw-matching rule for the provider-artifact family: a LITERAL
    // citation token in an autolink href fires P002...
    let text = "See <https://x/turn0search1> here.\n";
    let report = run(text, Profile::Doc);
    assert!(
        has_rule(&report, "SLOP-P002"),
        "a literal citation token in an autolink must fire: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);

    // ...but an entity-encoded spelling stays literal in the rendered href —
    // no functional token exists, so firing would be a false positive.
    let text = "See <https://x/turn0&#115;earch> here.\n";
    let report = run(text, Profile::Doc);
    assert!(
        !has_rule(&report, "SLOP-P002"),
        "an entity-encoded autolink token is not a functional token: {:?}",
        rule_ids(&report)
    );
    common::assert_invariants(text, &report);
}
