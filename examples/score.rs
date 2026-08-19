//! Guard measurement. Scores each file the way `analyze` does, using the
//! extractor's own code-block segmentation, and reports the ruled line test
//! beside two counterfactual extensions so a calibration decision has numbers.

use std::ops::Range;

fn code_blocks(text: &str) -> Vec<Range<usize>> {
    let config = unslop::Config::new(unslop::Profile::Doc);
    let prepared = unslop::input::prepare(text.as_bytes(), &config).unwrap();
    let doc = unslop::extract::build_doc(&prepared, &config).unwrap();
    doc.regions
        .iter()
        .filter(|r| r.kind == unslop::extract::RegionKind::CodeBlock)
        .map(|r| r.range.clone())
        .collect()
}

fn is_field_line(t: &str) -> bool {
    let Some(head) = t.strip_suffix(',') else {
        return false;
    };
    let head = head
        .strip_prefix("pub(crate) ")
        .or_else(|| head.strip_prefix("pub "))
        .unwrap_or(head);
    let Some((name, rest)) = head.split_once(':') else {
        return false;
    };
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && !rest.trim().is_empty()
}

fn score(text: &str, blocks: &[Range<usize>], comments: bool, fields: bool) -> (usize, usize) {
    let mut code = 0usize;
    let mut nonblank = 0usize;
    for lr in unslop::input::line_ranges(text) {
        let t = text[lr.clone()].trim();
        if t.is_empty() || blocks.iter().any(|c| c.start < lr.end && lr.start < c.end) {
            continue;
        }
        nonblank += 1;
        let ruled = unslop::input::source_line_counts(t, &[]).0 == 1;
        let extended = (comments && t.starts_with("//")) || (fields && is_field_line(t));
        if ruled || extended {
            code += 1;
        }
    }
    (code, nonblank)
}

fn main() {
    let mut paths: Vec<String> = std::env::args().skip(1).collect();
    paths.sort();
    println!("{:<40}{:>7}   counts   fires", "file", "score");
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let blocks = code_blocks(&text);
        let pct = |c: (usize, usize)| (c.0 * 100).checked_div(c.1).unwrap_or(0);
        let a = score(&text, &blocks, false, false);
        let fires = unslop::input::source_shape(&text, &blocks).is_some();
        let name = path.rsplit('/').next().unwrap_or(&path);
        println!(
            "{:<40}{:>6}%   {}/{}   {}",
            name,
            pct(a),
            a.0,
            a.1,
            if fires { "FIRES" } else { "silent" }
        );
    }
}
