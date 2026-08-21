//! One markdown parse via `into_offset_iter()`. The same walk builds prose
//! regions, the section tree, the segmentation map, and structural stats.
//! `Event::Code` and text inside `CodeBlock` are never prose.

use crate::input::{line_ranges, FormatData, Prepared};
use crate::{AnalysisError, Config};
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};
use std::ops::Range;

pub const F_QUOTED: u8 = 1;
pub const F_HEADING: u8 = 2;
/// A norm segment produced by the FULLY-FOLDABLE homoglyph path: the
/// whole token was cross-script confusables with no Latin witness, folded to
/// Latin for matching. The engine downgrades any match on such a segment to
/// CANDIDATE — the conservative tier for the rare genuine-foreign-word that
/// folds onto an English lexicon term.
pub const F_FULL_FOLD: u8 = 64;
/// Prose extracted from the visible text of a raw-HTML region. A browser
/// applies HTML reference grammar to this text — unbounded digits, optional
/// semicolon — so the numeric-reference anomaly scan holds it to the
/// stricter fail-closed rule for this evasion class.
pub const F_HTML_TEXT: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    Prose,
    CodeBlock,
    InlineCode,
    Html,
    Autolink,
    LinkUrl,
    Structure,
}

impl RegionKind {
    pub fn reason(self) -> &'static str {
        match self {
            RegionKind::Prose => "prose",
            RegionKind::CodeBlock => "code_fence",
            RegionKind::InlineCode => "inline_code",
            RegionKind::Html => "html",
            RegionKind::Autolink => "autolink",
            RegionKind::LinkUrl => "link_url",
            RegionKind::Structure => "structure",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Region {
    pub range: Range<usize>,
    pub kind: RegionKind,
}

#[derive(Debug, Clone)]
pub enum NormOp {
    /// Block boundary: paragraph, heading, list item, table cell, quote line.
    Block,
    Text {
        range: Range<usize>,
        flags: u8,
    },
    /// Owned prose whose source bytes differ from the value (escaped TOML
    /// strings). The whole range maps to the whole content.
    TextOwned {
        range: Range<usize>,
        content: String,
        flags: u8,
    },
    Break {
        range: Range<usize>,
        hard: bool,
        /// Same quote/heading flags as the Text and Barrier ops beside it:
        /// a softbreak between two wrapped blockquote (or setext heading)
        /// lines is inside the quote, and its norm segment must say so or
        /// `all_quoted` breaks across the wrap.
        flags: u8,
    },
    /// A word barrier for an excluded INLINE region whose rendered content
    /// visibly interrupts the surrounding prose — inline code (a code span
    /// renders at least one character; CommonMark cannot express an empty one)
    /// and autolink URLs. The norm view interposes U+FFFD so flanking text can
    /// never fuse into a word or phrase the reader does not see: `del` + code +
    /// `ve` must not assemble "delve". U+FFFD rather than a space or newline
    /// because a space would manufacture a two-word phrase (`game` + code +
    /// `changer` must not assemble "game changer") and a newline would
    /// manufacture a false block start mid-sentence.
    Barrier {
        range: Range<usize>,
        flags: u8,
    },
}

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: u32,
    pub range: Range<usize>,
    pub text: String,
    pub text_range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub title: String,
    pub level: u32,
    pub range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct CodeRegion {
    pub range: Range<usize>,
    pub fenced: bool,
    pub info: String,
}

#[derive(Debug, Clone)]
pub struct HtmlComment {
    pub range: Range<usize>,
    pub content: String,
    pub content_range: Range<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub word_count: u64,
    pub paragraph_words: Vec<u64>,
    pub bullets: u64,
    pub bullets_with_link: u64,
    pub task_bullets: u64,
    pub bold_label_items: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub ops: Vec<NormOp>,
    pub regions: Vec<Region>,
    pub prose_regions: Vec<(Range<usize>, u8)>,
    pub code_regions: Vec<CodeRegion>,
    pub link_url_regions: Vec<Range<usize>>,
    pub html_comments: Vec<HtmlComment>,
    pub headings: Vec<Heading>,
    pub sections: Vec<Section>,
    /// (range, inner text with emphasis markers trimmed)
    pub emphasis: Vec<(Range<usize>, String)>,
    /// Source ranges of bold runs only, for the boldface-density count.
    pub strong_runs: Vec<Range<usize>>,
    pub stats: Stats,
    /// Source ranges of leading bold labels in list items.
    pub bold_label_ranges: Vec<Range<usize>>,
    pub fence_unclosed: Option<Range<usize>>,
    /// An HTML comment opened with `<!--` that never closes before the block
    /// (and the document) ends. A structural anomaly, fed to SLOP-M005.
    pub html_unclosed_comment: Option<Range<usize>>,
    /// A `<script>`/`<style>` opened with no matching close before the block
    /// (and the document) ends. Its body runs to end-of-block/EOF and is
    /// swallowed by every scanner — the same fail-open a closed body avoids —
    /// so it is a structural anomaly fed to SLOP-M005, in parity with the
    /// unclosed comment.
    pub html_unclosed_script: Option<Range<usize>>,
    /// An enumerated HTML construct the hand-rolled tokenizer knowingly cannot
    /// render-faithfully parse: `<![CDATA[` outside a comment, a `--!>` comment
    /// terminator, a self-closing SKIP_BODY element (`<script/>` etc.), or a
    /// `<template>`. Rather than silently scan or skip it, fail closed as a
    /// structural anomaly (SLOP-M005) for adjudication. First occurrence wins.
    pub html_unparseable: Option<Range<usize>>,
    /// A numeric character reference the norm view refuses to decode: an
    /// in-bounds `&#…;` targeting an invisible/control/format codepoint (the
    /// evasion signature), or, in HTML-derived text, a
    /// `&#`+digits form a browser would decode but the CommonMark reference
    /// grammar rejects (overlong leading zeros, missing
    /// semicolon). Fail closed as SLOP-M005. First occurrence wins.
    pub numeric_ref_anomaly: Option<Range<usize>>,
    pub html_bytes: usize,
    /// Link destinations whose DECODED form differs from the raw source
    /// bytes, as `(raw destination region, decoded text)`. Backslash
    /// escapes and character references are resolved by the markdown parser
    /// (inline links, reference definitions) or by a browser inside the href
    /// attribute (autolinks), so a tracking parameter spelled `utm\_source`
    /// or `utm&#95;source` reaches the reader as `utm_source` while the raw
    /// region scan never sees it. The engine scans the decoded text in
    /// addition to the raw region and maps hits back onto the region.
    pub link_url_decoded: Vec<(Range<usize>, String)>,
    /// Unused link reference definitions: (span, title text if any)
    pub unused_refdefs: Vec<(Range<usize>, Option<String>)>,
    /// Raw text dropped by the render: script and style element content.
    pub dropped_html_text: Vec<Range<usize>>,
}

pub fn build_doc(prepared: &Prepared, _config: &Config) -> Result<Doc, AnalysisError> {
    let doc = match &prepared.format {
        FormatData::Markdown => build_markdown(&prepared.text),
        FormatData::Text => build_text(&prepared.text),
    };
    Ok(doc)
}

fn count_words(s: &str) -> u64 {
    let mut n = 0u64;
    let mut in_word = false;
    for c in s.chars() {
        let w = unicode_ident::is_xid_continue(c);
        if w && !in_word {
            n += 1;
        }
        in_word = w;
    }
    n
}

fn build_markdown(src: &str) -> Doc {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let parser = Parser::new_ext(src, opts);
    let refdefs: Vec<(String, Range<usize>, String, Option<String>)> = parser
        .reference_definitions()
        .iter()
        .map(|(label, def)| {
            (
                label.to_string(),
                def.span.clone(),
                def.dest.to_string(),
                def.title.as_ref().map(|t| t.to_string()),
            )
        })
        .collect();

    let mut doc = Doc::default();
    let mut excluded: Vec<Region> = Vec::new();

    let mut code_depth = 0usize;
    let mut quote_depth = 0usize;
    let mut heading_level: Option<u32> = None;
    let mut heading_buf = String::new();
    let mut heading_texts: Vec<Range<usize>> = Vec::new();
    let mut heading_range = 0..0;
    let mut autolink_depth = 0usize;
    let mut item_stack: Vec<bool> = Vec::new(); // has_link per open item
    let mut para_words: Option<u64> = None;
    let mut item_fresh = false;
    let mut pending_bold_label: Option<Range<usize>> = None;
    let mut awaiting_colon: Option<Range<usize>> = None;
    let mut used_labels: Vec<String> = Vec::new();
    // (opening fence range, fence char, opening run length, opening fence
    // column, info string)
    let mut fence_stack: Vec<(Range<usize>, char, usize, usize, String)> = Vec::new();
    // A run of consecutive block-HTML events pulldown emits line by line for
    // one HTML block, accumulated so `collect_html_payload` sees the JOINED
    // slice. Parsing each `Event::Html` fragment in isolation split a valid
    // multi-line `<!-- ... -->` comment across events and raised a spurious
    // unclosed-comment anomaly (see `collect_html_payload`).
    let mut pending_html: Option<Range<usize>> = None;

    for (event, range) in parser.into_offset_iter() {
        // Any non-HTML event ends the current HTML block: flush the joined run.
        // Exception: a ZERO-RANGE whitespace-only Text event is a pulldown
        // quirk (it emits one between the wrapped lines of a tab-indented
        // list-item HTML block) — it must NOT break the run, or a multi-line
        // comment is split, hiding its content from Y001 and leaking the tail
        // to a prose scan. The following gap then bridges normally.
        let ignorable_ws = matches!(&event, Event::Text(t)
            if range.start == range.end && t.chars().all(char::is_whitespace));
        if !matches!(event, Event::Html(_) | Event::InlineHtml(_)) && !ignorable_ws {
            if let Some(joined) = pending_html.take() {
                collect_html_payload(src, &joined, &mut doc);
            }
        }
        match event {
            Event::Start(tag) => {
                match &tag {
                    Tag::Paragraph
                    | Tag::Item
                    | Tag::TableCell
                    | Tag::FootnoteDefinition(_)
                    | Tag::MetadataBlock(_) => {
                        doc.ops.push(NormOp::Block);
                    }
                    Tag::Heading { .. } | Tag::BlockQuote(_) => {
                        doc.ops.push(NormOp::Block);
                    }
                    _ => {}
                }
                match tag {
                    Tag::Table(_) => {
                        // Leading table edge: prose immediately before a
                        // table renders in its own block, so it must not fuse
                        // into the first cell (a paragraph ending "--" plus a
                        // word cell assembled S001's signature shape across the
                        // table edge). Same U+FFFD mechanism as the cell-end
                        // barrier below; the range covers the table's first
                        // source char (always a char boundary).
                        let bend = src[range.start..]
                            .chars()
                            .next()
                            .map(|c| range.start + c.len_utf8())
                            .unwrap_or(range.start);
                        let mut flags = 0u8;
                        if quote_depth > 0 {
                            flags |= F_QUOTED;
                        }
                        doc.ops.push(NormOp::Barrier {
                            range: range.start..bend,
                            flags,
                        });
                    }
                    Tag::CodeBlock(kind) => {
                        code_depth += 1;
                        // The rendered block visibly interrupts the prose
                        // exactly as an inline code span does, so it gets
                        // the same U+FFFD barrier. Exclusion alone left NO
                        // trace in the norm view: the prose before a fence
                        // spliced directly against the prose after it, and
                        // a U001 run fused across DIFFERING fenced contents
                        // into a phantom duplicate. The barrier makes the
                        // gap a segment break for every rule. The range
                        // covers the block's first source char (always a
                        // char boundary) so the segment has real source
                        // bytes for trigger-fidelity reconstruction — same
                        // mechanism as the table leading edge above.
                        let bend = src[range.start..]
                            .chars()
                            .next()
                            .map(|c| range.start + c.len_utf8())
                            .unwrap_or(range.start);
                        let mut flags = 0u8;
                        if quote_depth > 0 {
                            flags |= F_QUOTED;
                        }
                        doc.ops.push(NormOp::Barrier {
                            range: range.start..bend,
                            flags,
                        });
                        let (fenced, info) = match kind {
                            CodeBlockKind::Fenced(i) => (true, i.to_string()),
                            CodeBlockKind::Indented => (false, String::new()),
                        };
                        if fenced {
                            let block = &src[range.clone()];
                            let opening = block.trim_start_matches([' ', '\t', '>']);
                            let ch = opening.chars().next().unwrap_or('`');
                            let open_len = opening.chars().take_while(|&c| c == ch).count();
                            // Column of the fence char on its own line (tab
                            // stops of 4), measured from the line start so the
                            // close check can subtract the same container
                            // prefix.
                            let fence_pos = range.start + (block.len() - opening.len());
                            let line_start =
                                src[..fence_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
                            let open_col = width_cols(&src[line_start..fence_pos]);
                            fence_stack.push((range.clone(), ch, open_len, open_col, info.clone()));
                        }
                        doc.code_regions.push(CodeRegion {
                            range: range.clone(),
                            fenced,
                            info,
                        });
                        excluded.push(Region {
                            range: range.clone(),
                            kind: RegionKind::CodeBlock,
                        });
                        item_fresh = false;
                    }
                    Tag::BlockQuote(_) => {
                        quote_depth += 1;
                    }
                    Tag::Heading { level, .. } => {
                        heading_level = Some(level as u32);
                        heading_buf.clear();
                        heading_texts.clear();
                        heading_range = range.clone();
                    }
                    Tag::Paragraph => {
                        para_words = Some(0);
                    }
                    Tag::Item => {
                        doc.stats.bullets += 1;
                        item_stack.push(false);
                        item_fresh = true;
                        pending_bold_label = None;
                        awaiting_colon = None;
                    }
                    Tag::Link {
                        link_type,
                        dest_url,
                        ref id,
                        ..
                    } => {
                        if !id.is_empty() {
                            used_labels.push(id.to_lowercase());
                        }
                        if let Some(top) = item_stack.last_mut() {
                            *top = true;
                        }
                        match link_type {
                            LinkType::Autolink | LinkType::Email => {
                                autolink_depth += 1;
                                // Inner text is the URL itself.
                                let inner = range.start + 1..range.end.saturating_sub(1);
                                if inner.start < inner.end {
                                    excluded.push(Region {
                                        range: inner.clone(),
                                        kind: RegionKind::Autolink,
                                    });
                                    doc.link_url_regions.push(inner.clone());
                                    // Autolink hrefs are matched RAW, never
                                    // decoded. CommonMark does not resolve
                                    // character references inside an autolink
                                    // URI — the renderer amp-escapes it, so
                                    // the browser href carries the literal
                                    // `&#95;` bytes and never a decoded
                                    // tracking token. Decoding here would
                                    // manufacture a false positive; only
                                    // inline links and refdefs, where the
                                    // parser really decodes, get a
                                    // link_url_decoded entry.
                                    // The URL renders visibly between the
                                    // flanking runs: same barrier as inline
                                    // code.
                                    let mut flags = 0u8;
                                    if quote_depth > 0 {
                                        flags |= F_QUOTED;
                                    }
                                    if heading_level.is_some() {
                                        flags |= F_HEADING;
                                    }
                                    doc.ops.push(NormOp::Barrier {
                                        range: inner,
                                        flags,
                                    });
                                }
                            }
                            LinkType::Inline => {
                                if let Some(rel) = inline_dest_range(&src[range.clone()], &dest_url)
                                {
                                    let url_range = range.start + rel.start..range.start + rel.end;
                                    excluded.push(Region {
                                        range: url_range.clone(),
                                        kind: RegionKind::LinkUrl,
                                    });
                                    if !url_range.is_empty()
                                        && src[url_range.clone()] != *dest_url
                                        && !dest_url.is_empty()
                                    {
                                        doc.link_url_decoded
                                            .push((url_range.clone(), dest_url.to_string()));
                                    }
                                    doc.link_url_regions.push(url_range);
                                }
                            }
                            LinkType::Reference
                            | LinkType::ReferenceUnknown
                            | LinkType::Collapsed
                            | LinkType::CollapsedUnknown
                            | LinkType::Shortcut
                            | LinkType::ShortcutUnknown => {}
                            _ => {}
                        }
                    }
                    Tag::Image {
                        link_type,
                        dest_url,
                        ref id,
                        ..
                    } => {
                        if !id.is_empty() {
                            used_labels.push(id.to_lowercase());
                        }
                        if link_type == LinkType::Inline {
                            if let Some(rel) = inline_dest_range(&src[range.clone()], &dest_url) {
                                let url_range = range.start + rel.start..range.start + rel.end;
                                excluded.push(Region {
                                    range: url_range.clone(),
                                    kind: RegionKind::LinkUrl,
                                });
                                if !url_range.is_empty()
                                    && src[url_range.clone()] != *dest_url
                                    && !dest_url.is_empty()
                                {
                                    doc.link_url_decoded
                                        .push((url_range.clone(), dest_url.to_string()));
                                }
                                doc.link_url_regions.push(url_range);
                            }
                        }
                        // The image renders as a replaced object; its
                        // alt text is visible-fallback prose (still scanned)
                        // that must not fuse with the flanking runs. One
                        // barrier on each side: here at the leading `![`, the
                        // closing side at TagEnd::Image.
                        let mut flags = 0u8;
                        if quote_depth > 0 {
                            flags |= F_QUOTED;
                        }
                        if heading_level.is_some() {
                            flags |= F_HEADING;
                        }
                        doc.ops.push(NormOp::Barrier {
                            range: range.start..(range.start + 2).min(range.end),
                            flags,
                        });
                    }
                    Tag::Emphasis | Tag::Strong => {
                        let inner = src[range.clone()].trim_matches(['*', '_']);
                        doc.emphasis.push((range.clone(), inner.to_string()));
                        if matches!(tag, Tag::Strong) {
                            doc.strong_runs.push(range.clone());
                            if item_fresh {
                                pending_bold_label = Some(range.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => match tag_end {
                TagEnd::CodeBlock => {
                    code_depth = code_depth.saturating_sub(1);
                    if let Some((frange, ch, open_len, open_col, _info)) = fence_stack.pop() {
                        if is_unclosed_fence(src, &frange, ch, open_len, open_col) {
                            doc.fence_unclosed = Some(frange);
                        }
                    }
                }
                TagEnd::BlockQuote(_) => {
                    quote_depth = quote_depth.saturating_sub(1);
                }
                TagEnd::Heading(_) => {
                    let level = heading_level.take().unwrap_or(1);
                    let text_range = match (heading_texts.first(), heading_texts.last()) {
                        (Some(f), Some(l)) => f.start..l.end,
                        _ => heading_range.start..heading_range.start,
                    };
                    doc.headings.push(Heading {
                        level,
                        range: heading_range.clone(),
                        text: heading_buf.trim().to_string(),
                        text_range,
                    });
                }
                TagEnd::Paragraph => {
                    if let Some(w) = para_words.take() {
                        doc.stats.paragraph_words.push(w);
                    }
                }
                TagEnd::Item => {
                    if let Some(had_link) = item_stack.pop() {
                        if had_link {
                            doc.stats.bullets_with_link += 1;
                        }
                    }
                    item_fresh = false;
                }
                TagEnd::Strong => {
                    if let Some(label_range) = pending_bold_label.take() {
                        // `**Label:** text` carries the colon inside the bold
                        // span; `**Label**: text` carries it just after.
                        if src[label_range.clone()]
                            .trim_end_matches(['*', '_'])
                            .trim_end()
                            .ends_with(':')
                        {
                            doc.stats.bold_label_items += 1;
                            doc.bold_label_ranges.push(label_range);
                        } else {
                            awaiting_colon = Some(label_range);
                        }
                    }
                }
                TagEnd::Link => {
                    autolink_depth = autolink_depth.saturating_sub(1);
                }
                TagEnd::TableCell => {
                    // Table cells render in separate boxes, so a match must
                    // never fuse across the `|` cell delimiter. The Block
                    // newline between cells is not enough: patterns with
                    // a `\s{1,N}` gap crossed it — S001's `^--\s{1,8}\S` and
                    // M001's `\s--\s` fired on placeholder-dash cells by
                    // pairing one cell's `--` with the NEXT cell's text. Same
                    // U+FFFD barrier as inline code: not a word char, not
                    // whitespace, not a line break, so flanking cells neither
                    // fuse into a word nor bridge a whitespace gap. The barrier
                    // sits at the cell END only — never at the cell start,
                    // where it would break the block-start position of the
                    // cell's own text and hide a genuine in-cell signature
                    // line. The range covers the delimiter char after the cell
                    // (`|`, or the line break on a pipeless last cell) so the
                    // segment has real source bytes for trigger-fidelity
                    // reconstruction.
                    let bend = src[range.end..]
                        .chars()
                        .next()
                        .map(|c| range.end + c.len_utf8())
                        .unwrap_or(range.end);
                    let mut flags = 0u8;
                    if quote_depth > 0 {
                        flags |= F_QUOTED;
                    }
                    doc.ops.push(NormOp::Barrier {
                        range: range.end..bend,
                        flags,
                    });
                }
                TagEnd::Image => {
                    // Closing side of the image-alt barrier pair.
                    let mut flags = 0u8;
                    if quote_depth > 0 {
                        flags |= F_QUOTED;
                    }
                    if heading_level.is_some() {
                        flags |= F_HEADING;
                    }
                    doc.ops.push(NormOp::Barrier {
                        range: range.end.saturating_sub(1)..range.end,
                        flags,
                    });
                }
                _ => {}
            },
            Event::Text(_) => {
                if code_depth > 0 {
                    // Code text stays inside the excluded code region.
                } else if autolink_depth > 0 {
                    // Autolink URL text, already excluded.
                } else {
                    let mut flags = 0u8;
                    if quote_depth > 0 {
                        flags |= F_QUOTED;
                    }
                    if heading_level.is_some() {
                        flags |= F_HEADING;
                        heading_buf.push_str(&src[range.clone()]);
                        heading_texts.push(range.clone());
                    }
                    let w = count_words(&src[range.clone()]);
                    doc.stats.word_count += w;
                    if let Some(pw) = para_words.as_mut() {
                        *pw += w;
                    }
                    if let Some(label_range) = awaiting_colon.take() {
                        if src[range.clone()].trim_start().starts_with(':') {
                            doc.stats.bold_label_items += 1;
                            doc.bold_label_ranges.push(label_range);
                        }
                    }
                    doc.ops.push(NormOp::Text {
                        range: range.clone(),
                        flags,
                    });
                    if !src[range.clone()].trim().is_empty() {
                        item_fresh = false;
                    }
                }
            }
            Event::Code(_) => {
                if code_depth == 0 {
                    excluded.push(Region {
                        range: range.clone(),
                        kind: RegionKind::InlineCode,
                    });
                    doc.code_regions.push(CodeRegion {
                        range: range.clone(),
                        fenced: false,
                        info: String::new(),
                    });
                    let mut flags = 0u8;
                    if quote_depth > 0 {
                        flags |= F_QUOTED;
                    }
                    if heading_level.is_some() {
                        flags |= F_HEADING;
                    }
                    doc.ops.push(NormOp::Barrier {
                        range: range.clone(),
                        flags,
                    });
                }
                item_fresh = false;
                awaiting_colon = None;
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                doc.html_bytes += range.len();
                excluded.push(Region {
                    range: range.clone(),
                    kind: RegionKind::Html,
                });
                // A RENDER-AFFECTING void tag
                // (`<br>`, `<img …>`, `<hr>`, …) puts a visible break or
                // object between the flanking runs, so they must not fuse —
                // `del<br>ve` is read as two fragments, never "delve".
                // Everything else fuses render-faithfully: formatting tags
                // (`del<b></b>ve` renders "delve" and must keep firing) and
                // the NON-rendering void tags (`del<wbr>ve` also renders
                // "delve"; barriering those would be an evasion channel —
                // see BARRIER_VOID_ELEMENTS).
                if matches!(&event, Event::InlineHtml(_)) {
                    let frag = &src[range.clone()];
                    if !frag.starts_with("</")
                        && BARRIER_VOID_ELEMENTS.contains(&tag_name(frag).as_str())
                    {
                        let mut flags = 0u8;
                        if quote_depth > 0 {
                            flags |= F_QUOTED;
                        }
                        if heading_level.is_some() {
                            flags |= F_HEADING;
                        }
                        doc.ops.push(NormOp::Barrier {
                            range: range.clone(),
                            flags,
                        });
                    }
                }
                // Accumulate one logical HTML block. pulldown emits blockquoted
                // and list-indented HTML line by line, each event's range
                // EXCLUDING the `> `/indent prefix, so the lines of one block
                // arrive as consecutive Html events separated by a gap that is
                // only a line break plus those markers. Bridge such a gap; a
                // gap with any other content (or a blank line) is a genuinely
                // separate block, so FLUSH the accumulated run before starting a
                // new one. Overwriting `pending_html` without flushing would
                // silently lose the previous fragment.
                match pending_html.take() {
                    Some(p)
                        if p.end == range.start || is_wrapped_html_gap(src, p.end..range.start) =>
                    {
                        pending_html = Some(p.start..range.end);
                    }
                    Some(p) => {
                        collect_html_payload(src, &p, &mut doc);
                        pending_html = Some(range.clone());
                    }
                    None => pending_html = Some(range.clone()),
                }
                item_fresh = false;
            }
            Event::SoftBreak | Event::HardBreak => {
                if code_depth == 0 {
                    let mut flags = 0u8;
                    if quote_depth > 0 {
                        flags |= F_QUOTED;
                    }
                    if heading_level.is_some() {
                        flags |= F_HEADING;
                    }
                    doc.ops.push(NormOp::Break {
                        range: range.clone(),
                        hard: matches!(&event, Event::HardBreak),
                        flags,
                    });
                }
            }
            Event::TaskListMarker(_) => {
                doc.stats.task_bullets += 1;
            }
            Event::FootnoteReference(_) => {
                // The marker is rendered ("del[^1]ve" reads as
                // del¹ve), so flanking runs must not fuse — same barrier as
                // inline code. A marker after a completed word ("delve[^1]")
                // still fires: U+FFFD is non-xid, so the boundary holds.
                let mut flags = 0u8;
                if quote_depth > 0 {
                    flags |= F_QUOTED;
                }
                if heading_level.is_some() {
                    flags |= F_HEADING;
                }
                doc.ops.push(NormOp::Barrier {
                    range: range.clone(),
                    flags,
                });
            }
            Event::Rule => {
                item_fresh = false;
            }
            _ => {}
        }
    }

    // Flush a trailing HTML block that ran to the end of the event stream.
    if let Some(joined) = pending_html.take() {
        collect_html_payload(src, &joined, &mut doc);
    }

    for (label, span, dest, title) in refdefs {
        // Only the DESTINATION substring is URL text. The whole span
        // (label and title included) stays excluded from prose, but scanning
        // the label as a URL fired P004 on label text over a clean
        // destination.
        let dest_range = refdef_dest_range(src, &span);
        // The parser resolves escapes/references in the destination;
        // scan the decoded form too when it differs from the raw bytes.
        if !dest_range.is_empty() && src[dest_range.clone()] != dest && !dest.is_empty() {
            doc.link_url_decoded.push((dest_range.clone(), dest));
        }
        doc.link_url_regions.push(dest_range);
        excluded.push(Region {
            range: span.clone(),
            kind: RegionKind::LinkUrl,
        });
        if !used_labels.iter().any(|u| u == &label.to_lowercase()) {
            doc.unused_refdefs.push((span, title));
        }
    }

    finish(src, &mut doc, excluded);
    doc
}

/// The RENDER-AFFECTING void elements: each puts something the
/// reader SEES between the flanking runs — `br`/`hr` a break, `img`/`embed` a
/// replaced box, `input` a form control — so it is a word barrier:
/// flanking prose must not fuse across it. Deliberately NOT the full HTML
/// void set: the non-rendering void tags (`meta`, `link`, `base`, `area`,
/// `col`, `param`, `source`, `track`, and especially `wbr`, which renders
/// nothing at all) leave the flanking text VISUALLY FUSED — `del<wbr>ve`
/// reads "delve" — so barriering them would hand an author a free
/// hide-a-lexicon-word channel, the same evasion class the homoglyph fold
/// closed. Those tags fuse, render-faithfully. Non-void formatting
/// tags (`<b>`, `<span>`, …) also stay transparent: `del<b></b>ve` really
/// renders "delve".
const BARRIER_VOID_ELEMENTS: &[&str] = &["br", "embed", "hr", "img", "input"];

/// Raw destination byte range inside an inline link/image span, relative to
/// the span. Two steps:
///
/// 1. Literal: the first occurrence of the PARSED destination after the real
///    `](` delimiter — skipping any `](` whose `]` is backslash-escaped, so a
///    label containing `\](` cannot claim the delimiter. (`rfind` over the
///    whole span picked the TITLE occurrence when the title repeats the URL.)
/// 2. Syntactic fallback: when the parsed destination has no literal
///    occurrence (its raw spelling carries backslash escapes or character
///    references the parser decoded), parse the destination grammar after the
///    delimiter — `<…>`-wrapped or bare to whitespace/the closing paren at
///    balance 0. Without the fallback this case would skip the region
///    entirely, leaving `utm\_source` / `utm&#95;source` destinations
///    silently unscanned.
fn inline_dest_range(slice: &str, dest: &str) -> Option<Range<usize>> {
    let delim = inline_delim(slice)?;
    if !dest.is_empty() {
        if let Some(p) = slice[delim..].find(dest) {
            return Some(delim + p..delim + p + dest.len());
        }
    }
    inline_dest_syntactic(slice.as_bytes(), delim)
}

/// Byte offset just past the first `](` whose `]` is not backslash-escaped
/// (escape parity: `\\](` is an escaped backslash followed by a REAL `]`).
fn inline_delim(slice: &str) -> Option<usize> {
    let mut at = 0usize;
    while let Some(o) = slice[at..].find("](") {
        let pos = at + o;
        let backslashes = slice[..pos]
            .bytes()
            .rev()
            .take_while(|&b| b == b'\\')
            .count();
        if backslashes % 2 == 0 {
            return Some(pos + 2);
        }
        at = pos + 1;
    }
    None
}

/// The inline-destination grammar after the `](` delimiter: optional
/// whitespace, then `<…>` (inner text, no newline) or a bare run ending at
/// ASCII whitespace or the closing `)` at parenthesis balance 0, with
/// backslash escapes consumed. All compared delimiters are ASCII, so byte
/// stepping cannot land a range endpoint off a char boundary.
fn inline_dest_syntactic(bytes: &[u8], delim: usize) -> Option<Range<usize>> {
    let mut d = delim;
    while d < bytes.len() && bytes[d].is_ascii_whitespace() {
        d += 1;
    }
    if d >= bytes.len() {
        return None;
    }
    if bytes[d] == b'<' {
        let inner = d + 1;
        let mut j = inner;
        loop {
            match bytes.get(j) {
                Some(b'\\') => j += 2,
                Some(b'>') => return Some(inner..j),
                Some(b'\n') | None => return None,
                Some(_) => j += 1,
            }
        }
    }
    let mut j = d;
    let mut depth = 0usize;
    while let Some(&b) = bytes.get(j) {
        match b {
            // Clamp so a trailing backslash cannot step `j` past the slice
            // end and hand back an out-of-bounds range.
            b'\\' => j = (j + 2).min(bytes.len()),
            b'(' => {
                depth += 1;
                j += 1;
            }
            b')' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                j += 1;
            }
            b if b.is_ascii_whitespace() => break,
            _ => j += 1,
        }
    }
    (d < j).then_some(d..j)
}

/// Destination byte range within a reference-definition span: the
/// grammar is `[label]: dest` with an optional title, and only the
/// destination is URL text — treating the whole span as a URL region fired
/// P004 on LABEL text over a clean destination. Any unexpected shape falls
/// back to the whole span, the fail-safe behavior. All compared
/// delimiters are ASCII, so byte stepping cannot leave a char boundary where
/// a range endpoint is taken.
fn refdef_dest_range(src: &str, span: &Range<usize>) -> Range<usize> {
    let slice = &src[span.clone()];
    let bytes = slice.as_bytes();
    // The label: `[` up to the first unescaped `]`, which must be followed
    // by `:`.
    let Some(open) = slice.find('[') else {
        return span.clone();
    };
    let mut i = open + 1;
    loop {
        match bytes.get(i) {
            Some(b'\\') => i += 2,
            Some(b']') => break,
            Some(_) => i += 1,
            None => return span.clone(),
        }
    }
    if bytes.get(i + 1) != Some(&b':') {
        return span.clone();
    }
    // Whitespace (the destination may sit on the next line).
    let mut d = i + 2;
    while d < bytes.len() && bytes[d].is_ascii_whitespace() {
        d += 1;
    }
    if d >= bytes.len() {
        return span.clone();
    }
    if bytes[d] == b'<' {
        // Angle-wrapped destination: the inner text up to the closing `>`.
        let inner = d + 1;
        let mut j = inner;
        loop {
            match bytes.get(j) {
                Some(b'\\') => j += 2,
                Some(b'>') => return span.start + inner..span.start + j,
                Some(b'\n') | None => return span.clone(),
                Some(_) => j += 1,
            }
        }
    }
    // Bare destination: runs to the next whitespace.
    let mut j = d;
    while j < bytes.len() && !bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    span.start + d..span.start + j
}

/// Column width of a line prefix with tab stops every 4 columns; every other
/// char (blockquote `>` markers included) advances one column.
fn width_cols(prefix: &str) -> usize {
    prefix.chars().fold(
        0,
        |col, c| if c == '\t' { col / 4 * 4 + 4 } else { col + 1 },
    )
}

fn is_unclosed_fence(
    src: &str,
    range: &Range<usize>,
    ch: char,
    open_len: usize,
    open_col: usize,
) -> bool {
    // A fence closed before EOF always has a closing line; only a block that
    // runs to the end of input can be unclosed.
    if range.end < src.len().saturating_sub(1) {
        return false;
    }
    let body = &src[range.clone()];
    let mut lines: Vec<&str> = body.lines().collect();
    while let Some(last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
        } else {
            break;
        }
    }
    if lines.len() < 2 {
        return true;
    }
    // CommonMark: the closing fence must use the same fence character, be a
    // line of only that character (optionally followed by whitespace), and
    // run at least as long as the opening fence. A shorter run — e.g. ``` for
    // a ````-opened block — does NOT close it, so the block runs to EOF and is
    // unclosed. A block whose non-fence tail is the last line lands here with
    // run == 0. This check must re-derive the opening length: otherwise a
    // 3-backtick line at EOF swallows a 4-backtick-opened block (run 3 >= 3),
    // hiding the slop tail as code. `ch`
    // is ASCII (backtick or tilde), so the run char count is a byte offset.
    // CommonMark allows a closing fence 0–3 columns of indentation RELATIVE
    // to its container. Measure the closing line's prefix in COLUMNS — a tab
    // advances to the next multiple of 4, quote markers count like any other
    // prefix char — then subtract the OPENING fence's column, which carries
    // the same container prefix shape. Stripping tabs and `>` before
    // counting would read `\t```` at EOF as 0 columns and false-close the
    // block (a fail-open), while a valid list-nested `  \t~~~` would
    // read its own spaces as indent plus a run-breaking tab and raise a
    // false unclosed-fence M005.
    let line = lines.last().unwrap();
    let tail = line.trim_start_matches([' ', '\t', '>']);
    let indent = width_cols(&line[..line.len() - tail.len()]).saturating_sub(open_col);
    let run = tail.chars().take_while(|&c| c == ch).count();
    let closes = indent <= 3 && run >= open_len.max(3) && tail[run..].trim().is_empty();
    !closes
}

/// True when the gap between two consecutive block-HTML events is only
/// blockquote (`>`) markers and list/indent whitespace — the shape pulldown
/// leaves between the per-line events of one wrapped HTML block, whose ranges
/// exclude the `> `/indent prefix. The line break belongs to the preceding
/// fragment, so a legitimate continuation gap carries NO newline (verified:
/// blockquote gap `"> "`, list gap `"  "`). Any line break here would be a
/// blank-line block boundary — which in practice arrives with an intervening
/// (non-HTML) event that already flushed the run — so it is not bridged.
fn is_wrapped_html_gap(src: &str, gap: Range<usize>) -> bool {
    if gap.start >= gap.end {
        return false;
    }
    let s = &src[gap];
    !s.contains('\n') && s.chars().all(|c| matches!(c, '>' | ' ' | '\t' | '\r'))
}

fn collect_html_payload(src: &str, range: &Range<usize>, doc: &mut Doc) {
    let slice = &src[range.clone()];
    let mut at = 0usize;
    while let Some(open) = slice[at..].find("<!--") {
        let cstart = at + open;
        let Some(close) = slice[cstart + 4..].find("-->") else {
            // An unclosed comment hides everything to end-of-block/EOF from
            // every scanner, which is strictly less protection than a closed
            // comment. Treat it as a structural anomaly (SLOP-M005) so the
            // malformed tail fails closed rather than silently vanishing.
            if doc.html_unclosed_comment.is_none() {
                doc.html_unclosed_comment = Some(range.start + cstart..range.end);
            }
            return;
        };
        let content_start = cstart + 4;
        let content_end = cstart + 4 + close;
        let content = slice[content_start..content_end].trim().to_string();
        doc.html_comments.push(HtmlComment {
            range: range.start + cstart..range.start + content_end + 3,
            content,
            content_range: range.start + content_start..range.start + content_end,
        });
        at = content_end + 3;
    }
    // Reader-visible text in a block-HTML region is prose the rendered page
    // shows but the markdown norm-view scan never saw. Extract it in SOURCE
    // coordinates and feed it into the norm view as prose so the ordinary AC +
    // regex prose passes cover it. Inline markup is
    // transparent — a word split across inline tags (`de<b></b>lve`) fuses —
    // while element-boundary whitespace renders as a space (`game <i>changer`
    // → `game changer`) and separate block elements get a hard boundary so
    // their text never fuses across a block edge.
    // Coverage-map note (accepted v1 inconsistency): these visible-text runs are
    // scanned as prose but the whole HTML region stays labeled `html` (excluded)
    // in the segmentation map, so such a finding can point into a region the
    // report calls excluded. Carving each run out of the html region would keep
    // the segmentation invariant (excluded + prose == total) only with careful
    // splitting and would shift coverage numbers; left as-is for v1.
    let mut anomaly = None;
    let pieces = html_visible_pieces(slice, range.start, &mut anomaly);
    if doc.html_unparseable.is_none() {
        doc.html_unparseable = anomaly;
    }
    if pieces.iter().any(|p| matches!(p, HtmlPiece::Text(_))) {
        doc.ops.push(NormOp::Block);
        for piece in pieces {
            match piece {
                HtmlPiece::Text(r) => {
                    // This visible run is now scanned as prose, so its
                    // bytes are NOT raw markup: subtract them from html_bytes so
                    // the SLOP-M005 raw-HTML-dominance test counts only genuine
                    // markup. Without this an idiomatic README (centered header +
                    // badges + table + <details>) tripped the 20% threshold.
                    doc.html_bytes = doc.html_bytes.saturating_sub(r.len());
                    doc.ops.push(NormOp::Text {
                        range: r,
                        flags: F_HTML_TEXT,
                    });
                }
                // A rendered inter-word space, mapped from the collapsed source
                // whitespace run so a multi-word slop term still matches. This
                // whitespace sits inside visible prose, so it is not raw markup
                // either — subtract it too (a text-heavy cell is ~15% spaces).
                HtmlPiece::Space(r) => {
                    doc.html_bytes = doc.html_bytes.saturating_sub(r.len());
                    doc.ops.push(NormOp::TextOwned {
                        range: r,
                        content: " ".to_string(),
                        flags: F_HTML_TEXT,
                    });
                }
                HtmlPiece::Block => doc.ops.push(NormOp::Block),
            }
        }
    }
    // Script/style bodies are dropped by the render (SLOP-Y001). Extract EVERY
    // body (not just the first) with a quote-aware tag end, and treat a body
    // that never closes before end-of-block/EOF as a structural anomaly
    // (SLOP-M005), in parity with the unclosed comment.
    collect_script_style_bodies(slice, range.start, doc);
}

/// A visible-text piece of an HTML block in SOURCE coordinates.
enum HtmlPiece {
    /// A maximal run of visible non-whitespace text.
    Text(Range<usize>),
    /// A rendered inter-word space, carrying the collapsed source-whitespace
    /// range it stands for.
    Space(Range<usize>),
    /// A block-element boundary: text before and after must not fuse.
    Block,
}

/// Inline (text-transparent) HTML elements: markup that does not introduce a
/// visible-text boundary. Everything not listed here is treated as a
/// block-level boundary so text on either side never fuses into a spurious
/// multi-word match. `pre`/`code`/`kbd`/`samp`/`script`/`style` are handled
/// separately (their bodies are skipped, not rendered as prose).
const INLINE_ELEMENTS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "cite", "data", "dfn", "em", "i", "mark", "q", "rp", "rt",
    "ruby", "s", "small", "span", "strong", "sub", "sup", "time", "u", "var", "wbr", "big", "tt",
    "ins", "del", "font", "nobr", "label", "img",
];

/// Elements whose body is code, not prose: skipped wholesale like script/style
/// so code embedded in HTML is never scanned as prose (parity with markdown
/// fences and the crate's "code never prose" invariant).
const SKIP_BODY_ELEMENTS: &[&str] = &["pre", "code", "kbd", "samp", "script", "style"];

/// Byte index just past the `>` that ends the tag beginning at `lt`, honoring
/// quoted attribute values: a `>` inside a `"…"` or `'…'` value does not end
/// the tag. `None` if no closing `>` exists. Quotes and `>` are ASCII, so byte
/// indexing stays on char boundaries.
fn tag_end(slice: &str, lt: usize) -> Option<usize> {
    let bytes = slice.as_bytes();
    let mut i = lt + 1;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i + 1),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// The lowercased ASCII element name of a tag slice like `<div …>` or `</div>`.
/// A `-` is part of the name (custom elements like `<code-sample>`): stopping at
/// the hyphen would misread `<code-sample>` as `<code>` and skip its body as
/// code. Chrome parses it as an ordinary unknown element whose text IS
/// visible, which this matches.
fn tag_name(tag: &str) -> String {
    tag.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Whether a complete tag slice (`<name …>`) uses self-closing syntax. A `/`
/// counts only when it immediately precedes the `>` AND is not the tail of an
/// unquoted attribute value: in `<script data-x=/ >` and `<a href=foo/>` the
/// `/` belongs to a VALUE, not the tag syntax. The token
/// holding the `/` must carry no `=` — a bare tag name (`<script/>`), a bare
/// attribute name (`<input disabled/>`), or nothing after a closing quote
/// (`<img src="x"/>`).
fn tag_is_self_closing(tag: &str) -> bool {
    let Some(head) = tag.strip_suffix("/>") else {
        return false;
    };
    let token_start = head
        .rfind(|c: char| c.is_ascii_whitespace() || c == '"' || c == '\'')
        .map(|p| p + 1)
        .unwrap_or(0);
    !head[token_start..].contains('=')
}

/// Earliest offset in `hay` of a `</name` close tag whose name ends at a real
/// boundary (`>`, `/`, whitespace, or end of input), ASCII case-insensitive —
/// `</prefix>` must not close `<pre>`.
fn find_close_tag(hay: &str, name: &str) -> Option<usize> {
    find_tag_ci(hay, &format!("</{name}"))
}

/// Earliest offset in `hay` of a `<name` open tag with the same name-boundary
/// rule, ASCII case-insensitive.
fn find_open_tag_ci(hay: &str, name: &str) -> Option<usize> {
    find_tag_ci(hay, &format!("<{name}"))
}

fn find_tag_ci(hay: &str, needle: &str) -> Option<usize> {
    let mut at = 0usize;
    while let Some(o) = find_ci_ascii(&hay[at..], needle.as_bytes()) {
        let pos = at + o;
        match hay.as_bytes().get(pos + needle.len()) {
            None => return Some(pos),
            Some(&c) if c == b'>' || c == b'/' || c.is_ascii_whitespace() => return Some(pos),
            _ => at = pos + 1,
        }
    }
    None
}

/// ASCII-case-insensitive prefix test.
fn starts_with_ci(s: &str, prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// The hidden-content anomaly triggers, scanned FLAT over the content of a
/// skipped code-bearing body: a construct that hides content from a
/// reader must fail closed even when nested where the prose scan never looks
/// (`<code><template>…</template></code>`). Flat substring scans — not a
/// recursive tokenize — so a crafted deep nesting cannot grow the stack; a
/// code body that legitimately QUOTES one of these constructs anomaly-flags,
/// which is the accepted fail-closed cost. Returns the source range of the
/// earliest trigger.
fn skipped_body_anomaly(body: &str, base: usize) -> Option<Range<usize>> {
    let mut first: Option<usize> = None;
    let mut consider = |p: Option<usize>| {
        if let Some(p) = p {
            first = Some(first.map_or(p, |f| f.min(p)));
        }
    };
    consider(find_ci_ascii(body, b"<![cdata["));
    consider(body.find("--!>"));
    consider(find_open_tag_ci(body, "template"));
    // A nested self-closing skip element (`<script/>` etc.).
    for name in SKIP_BODY_ELEMENTS {
        let mut at = 0usize;
        while let Some(o) = find_open_tag_ci(&body[at..], name) {
            let pos = at + o;
            let Some(te) = tag_end(body, pos) else { break };
            if tag_is_self_closing(&body[pos..te]) {
                consider(Some(pos));
                break;
            }
            at = te;
        }
    }
    first.map(|p| base + p..base + body.len())
}

/// HTML5 tag-start rule: a `<` begins a tag only when immediately followed by
/// an ASCII letter, `/`, `!`, or `?`. Any other `<` (e.g. `< delve`, `<3`) is
/// literal text.
fn is_tag_start(bytes: &[u8], i: usize) -> bool {
    matches!(bytes.get(i + 1), Some(&c) if c.is_ascii_alphabetic() || c == b'/' || c == b'!' || c == b'?')
}

/// Extract the visible-text pieces of an HTML slice in SOURCE coordinates
/// (`base` is the slice's source start). Comments, tag markup, and
/// code-bearing element bodies are removed; inline markup is transparent and
/// block-element edges become hard boundaries. Never lowercases the slice —
/// `to_lowercase` can change byte length and these offsets are source
/// coordinates, so tag names are compared case-insensitively on the original
/// bytes.
fn html_visible_pieces(
    slice: &str,
    base: usize,
    anomaly: &mut Option<Range<usize>>,
) -> Vec<HtmlPiece> {
    let bytes = slice.as_bytes();
    let n = slice.len();
    let mut pieces: Vec<HtmlPiece> = Vec::new();
    let mut run_start: Option<usize> = None;
    // The collapsed source range of whitespace pending before the next text.
    let mut pending_ws: Option<Range<usize>> = None;
    // Whether any text has been emitted in the current block (a Space is only
    // emitted between two texts within a block, never at a block edge).
    let mut text_in_block = false;
    let mut i = 0usize;

    let flush_run = |pieces: &mut Vec<HtmlPiece>, run_start: &mut Option<usize>, end: usize| {
        if let Some(s) = run_start.take() {
            if s < end {
                pieces.push(HtmlPiece::Text(base + s..base + end));
            }
        }
    };

    while i < n {
        if slice[i..].starts_with("<!--") {
            flush_run(&mut pieces, &mut run_start, i);
            match slice[i + 4..].find("-->") {
                Some(end) => {
                    // A `--!>` inside the comment closes it in a browser but not
                    // here (this scans to `-->`), so the tokenizer would hide
                    // text a reader sees. Fail closed for adjudication.
                    if anomaly.is_none() && slice[i + 4..i + 4 + end].contains("--!>") {
                        *anomaly = Some(base + i..base + i + 4 + end + 3);
                    }
                    i = i + 4 + end + 3;
                }
                None => break, // unclosed comment: nothing visible follows
            }
            continue;
        }
        if starts_with_ci(&slice[i..], b"<![CDATA[") {
            // CDATA outside a comment — matched case-insensitively, since a
            // browser recovers `<![cdata[` into the same swallowed-markup
            // shape: the tokenizer swallows its content as tag
            // markup (unscanned), so a slop term inside vanishes silently. Fail
            // closed.
            if anomaly.is_none() {
                *anomaly = Some(base + i..base + n);
            }
            break;
        }
        if bytes[i] == b'<' && is_tag_start(bytes, i) {
            flush_run(&mut pieces, &mut run_start, i);
            let Some(te) = tag_end(slice, i) else {
                break; // malformed tag runs to EOF
            };
            let tag = &slice[i..te];
            let name = tag_name(tag);
            let is_end = tag.starts_with("</");
            let self_closing = tag_is_self_closing(tag);
            if !is_end && name == "template" {
                // <template> content is inert (never rendered): the tokenizer
                // cannot decide whether it is prose. Fail closed and
                // skip the body so it is not scanned as prose either way.
                if anomaly.is_none() {
                    *anomaly = Some(base + i..base + n);
                }
                match find_close_tag(&slice[te..], "template") {
                    Some(c) => i = tag_end(slice, te + c).unwrap_or(n),
                    None => break,
                }
                if text_in_block {
                    pieces.push(HtmlPiece::Block);
                    text_in_block = false;
                }
                pending_ws = None;
                continue;
            }
            if !is_end && self_closing && SKIP_BODY_ELEMENTS.contains(&name.as_str()) {
                // `<script/>` etc.: self-closing syntax on a raw-text element is
                // a parse error browsers recover from unpredictably. Fail closed
                // rather than guess whether the body is scanned.
                if anomaly.is_none() {
                    *anomaly = Some(base + i..base + n);
                }
            }
            if !is_end && !self_closing && SKIP_BODY_ELEMENTS.contains(&name.as_str()) {
                // Skip the code-bearing body to its matching close tag; if it
                // never closes, nothing after it is visible.
                let body_end = match find_close_tag(&slice[te..], &name) {
                    Some(c) => te + c,
                    None => n,
                };
                // The skipped body is never scanned as prose, but a
                // hidden-content construct nested inside it must still fail
                // closed: run the same trigger scan over the
                // body content.
                if anomaly.is_none() {
                    *anomaly = skipped_body_anomaly(&slice[te..body_end], base + te);
                }
                if body_end == n {
                    break;
                }
                i = tag_end(slice, body_end).unwrap_or(n);
                // A skipped code body is a hard boundary.
                if text_in_block {
                    pieces.push(HtmlPiece::Block);
                    text_in_block = false;
                }
                pending_ws = None;
                continue;
            }
            if !INLINE_ELEMENTS.contains(&name.as_str()) {
                // Block-level boundary: flush and forbid cross-edge fusion.
                if text_in_block {
                    pieces.push(HtmlPiece::Block);
                    text_in_block = false;
                }
                pending_ws = None;
            }
            // Inline markup is transparent: pending whitespace (if any) still
            // separates the surrounding text, and no boundary is inserted.
            i = te;
            continue;
        }
        let ch = slice[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            flush_run(&mut pieces, &mut run_start, i);
            pending_ws = Some(match pending_ws.take() {
                Some(r) => r.start..base + i + ch.len_utf8(),
                None => base + i..base + i + ch.len_utf8(),
            });
            i += ch.len_utf8();
            continue;
        }
        // Visible non-whitespace char (a literal `<` that is not a tag start
        // lands here too).
        if run_start.is_none() {
            if text_in_block {
                if let Some(ws) = pending_ws.take() {
                    pieces.push(HtmlPiece::Space(ws));
                }
            }
            pending_ws = None;
            run_start = Some(i);
            text_in_block = true;
        }
        i += ch.len_utf8();
    }
    flush_run(&mut pieces, &mut run_start, n);
    pieces
}

/// Locate every `<script>`/`<style>` body in an HTML slice and record it as
/// render-dropped text (SLOP-Y001), using a quote-aware tag end so a `>` inside
/// an attribute value never leaks attribute bytes into the body span. A body
/// that never closes before end-of-block/EOF is a structural anomaly
/// (SLOP-M005). `base` is the slice's source start. Tag names are ASCII, so
/// the case-insensitive byte scan stays on char boundaries.
fn collect_script_style_bodies(slice: &str, base: usize, doc: &mut Doc) {
    let mut at = 0usize;
    while at < slice.len() {
        // Earliest of the next <script / <style open at or after `at`.
        let next = [
            ("script", b"<script".as_slice()),
            ("style", b"<style".as_slice()),
        ]
        .iter()
        .filter_map(|(name, needle)| find_ci_ascii(&slice[at..], needle).map(|o| (at + o, *name)))
        .min_by_key(|&(o, _)| o);
        let Some((o, name)) = next else { break };
        // Require a real tag start: the char after the name is `>`, `/`, or
        // whitespace (rejects e.g. `<scripting`).
        let after_name = o + 1 + name.len();
        let boundary = slice
            .as_bytes()
            .get(after_name)
            .map(|&c| c == b'>' || c == b'/' || c.is_ascii_whitespace())
            .unwrap_or(false);
        if !boundary {
            at = o + 1;
            continue;
        }
        let Some(te) = tag_end(slice, o) else {
            // Malformed open tag runs to EOF: unclosed body.
            if doc.html_unclosed_script.is_none() {
                doc.html_unclosed_script = Some(base + o..base + slice.len());
            }
            break;
        };
        // Boundary-checked close: `</scriptx>` does not end a
        // script body — matching HTML's raw-text end-tag rule.
        match find_close_tag(&slice[te..], name) {
            Some(c) => {
                let body = te..te + c;
                if !slice[body.clone()].trim().is_empty() {
                    doc.dropped_html_text
                        .push(base + body.start..base + body.end);
                }
                let close_lt = te + c;
                at = tag_end(slice, close_lt).unwrap_or(slice.len());
            }
            None => {
                // Unclosed body swallows the tail to end-of-block/EOF.
                if doc.html_unclosed_script.is_none() {
                    doc.html_unclosed_script = Some(base + o..base + slice.len());
                }
                break;
            }
        }
    }
}

/// First byte offset in `hay` at which an ASCII-lowercase `needle` matches
/// case-insensitively, or `None`. The match window is all ASCII, so the offset
/// is always a char boundary.
fn find_ci_ascii(hay: &str, needle: &[u8]) -> Option<usize> {
    let hb = hay.as_bytes();
    let m = needle.len();
    if m == 0 || hb.len() < m {
        return None;
    }
    (0..=hb.len() - m).find(|&j| hb[j..j + m].eq_ignore_ascii_case(needle))
}

/// Byte length of the list, quote, or heading marker opening `line`, counting
/// the whitespace on either side of it, or zero when the line opens on content.
/// Markdown mode has a parser that strips these and text mode has none, so a
/// marker-led line used to carry its marker into the prose range and every rule
/// anchored to a block start read the position after the marker rather than the
/// position a reader sees. A marker with no space after it is ordinary text, so
/// a horizontal rule, a negative number, and a hashtag all score zero here.
/// The bullet glyphs come from `views::BULLET_MARKERS`, the fleet-wide set
/// this shares with the block-start decoration table.
fn marker_run_len(line: &str) -> usize {
    let lead = line.len() - line.trim_start().len();
    let rest = &line[lead..];
    let marker = match rest.chars().next() {
        Some(c) if crate::views::BULLET_MARKERS.contains(&c) => c.len_utf8(),
        Some(c @ ('-' | '*' | '+' | '>')) => c.len_utf8(),
        Some('#') => rest.chars().take_while(|c| *c == '#').count(),
        Some(c) if c.is_ascii_digit() => {
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            match rest[digits..].chars().next() {
                Some('.') | Some(')') => digits + 1,
                _ => return 0,
            }
        }
        _ => return 0,
    };
    let after = &rest[marker..];
    let gap = after.len() - after.trim_start().len();
    if gap == 0 {
        return 0;
    }
    lead + marker + gap
}

fn build_text(src: &str) -> Doc {
    let mut doc = Doc::default();
    let lines = line_ranges(src);
    let mut prev_blank = true;
    let mut para_words = 0u64;
    for lr in &lines {
        let line = &src[lr.clone()];
        let blank = line.trim().is_empty();
        if blank {
            if para_words > 0 {
                doc.stats.paragraph_words.push(para_words);
                para_words = 0;
            }
            prev_blank = true;
            continue;
        }
        doc.ops.push(NormOp::Block);
        // The marker is structure, not prose. Opening the range past it puts
        // the block start where the reader sees it, and leaves the marker
        // bytes to be reported as excluded structure like any other.
        doc.ops.push(NormOp::Text {
            range: lr.start + marker_run_len(line)..lr.end,
            flags: 0,
        });
        let trimmed = line.trim_start();
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            doc.stats.bullets += 1;
            if trimmed.contains("](") || trimmed.contains("http") {
                doc.stats.bullets_with_link += 1;
            }
        }
        let w = count_words(line);
        doc.stats.word_count += w;
        para_words += w;
        prev_blank = false;
    }
    let _ = prev_blank;
    if para_words > 0 {
        doc.stats.paragraph_words.push(para_words);
    }
    let excluded = Vec::new();
    finish(src, &mut doc, excluded);
    doc
}

/// Byte length of a `&#`-digit run a BROWSER would parse as a numeric
/// character reference — any digit count, `;` optional (HTML terminates a
/// missing-semicolon reference at the first non-digit). Used only to
/// RECOGNIZE valid-HTML-but-undecodable refs in HTML-derived text; decoding
/// stays bounded by the CommonMark grammar in `views::classify_numeric_ref`.
fn html_numeric_ref_len(s: &str, amp: usize) -> Option<usize> {
    let body = s[amp..].strip_prefix("&#")?;
    let (digits, prefix, hex) = match body.strip_prefix(['x', 'X']) {
        Some(h) => (h, 3usize, true),
        None => (body, 2usize, false),
    };
    let run = digits
        .bytes()
        .take_while(|b| {
            if hex {
                b.is_ascii_hexdigit()
            } else {
                b.is_ascii_digit()
            }
        })
        .count();
    if run == 0 {
        return None;
    }
    let semi = digits.as_bytes().get(run) == Some(&b';');
    Some(prefix + run + usize::from(semi))
}

/// Classify every numeric character reference in
/// prose exactly as `views::push_text` will, and record the fail-closed
/// cases as a structural anomaly. `Suppress` (a ref targeting an invisible/
/// control/format codepoint) anomalies anywhere — no legitimate document
/// writes `&#8203;`. `Overlong` and semicolonless digit refs anomaly only in
/// HTML-derived text, where a browser decodes what the CommonMark grammar
/// leaves literal; in markdown prose the reader sees them literally, so
/// nothing hides.
fn scan_numeric_ref_anomalies(src: &str, doc: &mut Doc) {
    let mut anomaly: Option<Range<usize>> = None;
    'ops: for op in &doc.ops {
        let NormOp::Text { range, flags } = op else {
            continue;
        };
        let html = flags & F_HTML_TEXT != 0;
        let slice = &src[range.clone()];
        let bytes = slice.as_bytes();
        let mut i = 0usize;
        while i < slice.len() {
            if bytes[i] != b'&' {
                i += 1;
                continue;
            }
            // Mirror push_text's precedence: the enumerated entity table owns
            // its spellings (`&#8212;`, `&#160;`, …) before classification.
            if let Some((elen, _)) = crate::views::entity_at(slice, i) {
                i += elen;
                continue;
            }
            use crate::views::NumRef;
            let (len, bad) = match crate::views::classify_numeric_ref(slice, i) {
                NumRef::Decode(len, _) => (len, false),
                NumRef::Suppress(len) => (len, true),
                NumRef::Overlong(len) => (len, html),
                NumRef::Literal => match html_numeric_ref_len(slice, i) {
                    Some(len) => (len, html),
                    None => (1, false),
                },
            };
            if bad {
                anomaly = Some(range.start + i..range.start + i + len);
                break 'ops; // first occurrence wins
            }
            i += len;
        }
    }
    doc.numeric_ref_anomaly = anomaly;
}

/// Merge prose ranges, resolve overlaps deterministically, and fill gaps so
/// the segmentation covers 100% of the input with no overlap.
fn finish(src: &str, doc: &mut Doc, excluded: Vec<Region>) {
    scan_numeric_ref_anomalies(src, doc);
    // Prose regions from ops.
    let mut prose: Vec<(Range<usize>, u8)> = Vec::new();
    for op in &doc.ops {
        if let NormOp::Text { range, flags } | NormOp::TextOwned { range, flags, .. } = op {
            if range.start < range.end {
                match prose.last_mut() {
                    Some((last, f)) if last.end == range.start && *f == *flags => {
                        last.end = range.end;
                    }
                    _ => prose.push((range.clone(), *flags)),
                }
            }
        }
    }
    doc.prose_regions = prose;

    let mut all: Vec<Region> = Vec::new();
    for (r, _) in &doc.prose_regions {
        all.push(Region {
            range: r.clone(),
            kind: RegionKind::Prose,
        });
    }
    all.extend(excluded);
    all.sort_by_key(|r| (r.range.start, r.range.end));

    let mut resolved: Vec<Region> = Vec::new();
    for mut r in all {
        if let Some(prev) = resolved.last() {
            if r.range.start < prev.range.end {
                r.range.start = prev.range.end;
            }
        }
        if r.range.start < r.range.end {
            resolved.push(r);
        }
    }
    // Fill gaps with structure.
    let mut full: Vec<Region> = Vec::new();
    let mut at = 0usize;
    for r in resolved {
        if r.range.start > at {
            full.push(Region {
                range: at..r.range.start,
                kind: RegionKind::Structure,
            });
        }
        at = at.max(r.range.end);
        full.push(r);
    }
    if at < src.len() {
        full.push(Region {
            range: at..src.len(),
            kind: RegionKind::Structure,
        });
    }
    doc.regions = full;

    // Section tree from headings.
    let mut sections = Vec::new();
    for (i, h) in doc.headings.iter().enumerate() {
        let end = doc
            .headings
            .iter()
            .skip(i + 1)
            .find(|h2| h2.level <= h.level)
            .map(|h2| h2.range.start)
            .unwrap_or(src.len());
        sections.push(Section {
            title: h.text.clone(),
            level: h.level,
            range: h.range.start..end,
        });
    }
    doc.sections = sections;
}
