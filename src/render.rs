//! The locally computed rendered view. No network, no external renderer:
//! the comparison is derived from the same pulldown-cmark parse recorded in
//! the extraction walk.

use crate::engine::Hit;
use crate::extract::Doc;
use crate::input::Prepared;
use crate::Config;

/// SLOP-Y001: text present in source but absent from the rendered page.
/// Channels: HTML comments with content, script and style element bodies,
/// unused link reference definitions carrying prose (a title).
pub fn render_invisible(
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    rule_idx: usize,
    hits: &mut Vec<Hit>,
) {
    let _ = prepared;
    for c in &doc.html_comments {
        if c.content.is_empty() {
            continue;
        }
        let exempt = config
            .deployment
            .exempt_comment_markers
            .iter()
            .any(|m| c.content.starts_with(m.as_str()));
        if exempt {
            continue;
        }
        hits.push(Hit::new(rule_idx, c.range.clone()));
    }
    for r in &doc.dropped_html_text {
        hits.push(Hit::new(rule_idx, r.clone()));
    }
    for (span, title) in &doc.unused_refdefs {
        if title
            .as_ref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
        {
            hits.push(Hit::new(rule_idx, span.clone()));
        }
    }
}

/// SLOP-Y002: material divergence between the norm text and the rendered
/// text. The prose walk already places image alt text and link text in both
/// views, so the implemented divergence channel is narrow: unused reference
/// definitions without a title (prose the renderer drops without carrying a
/// finding under Y001).
pub fn render_divergence(
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    rule_idx: usize,
    hits: &mut Vec<Hit>,
) {
    let _ = (prepared, config);
    for (span, title) in &doc.unused_refdefs {
        if title.is_none() {
            hits.push(Hit::new(rule_idx, span.clone()));
        }
    }
    // Reader-visible text inside HTML blocks is no longer a Y002 divergence:
    // it is extracted in the norm view and scanned by the ordinary prose passes,
    // so slop there raises its own prose finding and
    // legitimate HTML text stays clean. Reporting it here too would both
    // double-count and fire on innocent tables/`<details>` blocks.
}
