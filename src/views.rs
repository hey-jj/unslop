//! The norm view: one owned String plus a run-length
//! `(norm_range -> source_range)` segment table resolved by binary search.
//! Applied in order: NFC normalization, enumerated entity decoding, markdown
//! escape resolution, default-ignorable (invisible) removal, soft break
//! folding.

use crate::extract::{Doc, NormOp};
use std::ops::Range;
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegKind {
    /// Norm bytes equal source bytes; offsets map linearly.
    Identity,
    /// Norm bytes differ from source bytes; a match touching any part of
    /// this segment maps to the whole source range.
    Mapped,
}

#[derive(Debug, Clone)]
pub struct Seg {
    pub norm: Range<usize>,
    pub src: Range<usize>,
    pub kind: SegKind,
    pub flags: u8,
}

#[derive(Debug, Default)]
pub struct NormView {
    pub text: String,
    pub segs: Vec<Seg>,
    /// Norm offsets that begin a block or a hard-broken line. A soft break
    /// inside a wrapped paragraph is not one: the reader never sees it.
    pub line_starts: Vec<usize>,
    /// Norm offsets that begin a block: a paragraph, heading, list item,
    /// table cell, or quote line. A wrapped paragraph is one block, which is
    /// the unit the sentence-shape rules read.
    pub block_starts: Vec<usize>,
    /// Invisible (default-ignorable) characters removed, as source offsets.
    pub zero_width_removed: Vec<usize>,
}

/// Enumerated entity set the policy patterns can reach. Never the full
/// HTML5 table.
const ENTITIES: &[(&str, &str)] = &[
    ("&mdash;", "\u{2014}"),
    ("&#8212;", "\u{2014}"),
    ("&#x2014;", "\u{2014}"),
    ("&ndash;", "\u{2013}"),
    ("&#8211;", "\u{2013}"),
    ("&#x2013;", "\u{2013}"),
    ("&hellip;", "\u{2026}"),
    ("&nbsp;", " "),
    ("&#160;", " "),
    ("&emsp;", " "),
    ("&ensp;", " "),
    ("&thinsp;", " "),
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&#34;", "\""),
    ("&apos;", "'"),
    ("&#39;", "'"),
    ("&semi;", ";"),
    ("&#59;", ";"),
    ("&ldquo;", "\u{201C}"),
    ("&rdquo;", "\u{201D}"),
    ("&lsquo;", "\u{2018}"),
    ("&rsquo;", "\u{2019}"),
    ("&#8220;", "\u{201C}"),
    ("&#8221;", "\u{201D}"),
    ("&#8216;", "\u{2018}"),
    ("&#8217;", "\u{2019}"),
];

/// Unicode `Default_Ignorable_Code_Point` (DerivedCoreProperties): every
/// codepoint a conformant renderer shows as NOTHING when unsupported — soft
/// hyphen, CGJ, bidi marks/embeddings/isolates, Mongolian and Khmer inherent
/// controls, Hangul fillers, variation selectors, word joiners, BOM,
/// interlinear annotation, shorthand/musical formats, and the plane-14 tag
/// block. The literal-path removal keys on this FULL set, not the
/// 5-char zero-width list, so no invisible character — literal or decoded —
/// can sit inside a lexicon word and hide it (`del\u{00AD}ve` normalizes to
/// "delve" and matches directly). Removal is render-faithful: a reader never
/// sees these.
const DEFAULT_IGNORABLE_RANGES: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF0, 0xFFF8),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE0FFF),
];

fn is_default_ignorable(ch: char) -> bool {
    let c = ch as u32;
    DEFAULT_IGNORABLE_RANGES
        .iter()
        .any(|&(lo, hi)| lo <= c && c <= hi)
}

/// Cross-script Latin homoglyphs: the realistic Cyrillic and Greek letters
/// that render identically (or near-identically) to Latin ones — the
/// homoglyph-evasion alphabet, deliberately NOT the full Unicode confusables
/// data file. Folded to Latin in the norm view ONLY inside a mixed-script
/// token: `dеlve` with a Cyrillic е normalizes to "delve" and the
/// lexicon fires directly, while a pure-Cyrillic or pure-Greek word —
/// genuine Russian or Greek text — is never touched.
const CONFUSABLE_TO_LATIN: &[(char, &str)] = &[
    // Cyrillic lowercase.
    ('а', "a"),
    ('в', "b"),
    ('е', "e"),
    ('к', "k"),
    ('м', "m"),
    ('н', "h"),
    ('о', "o"),
    ('р', "p"),
    ('с', "c"),
    ('т', "t"),
    ('у', "y"),
    ('х', "x"),
    ('і', "i"),
    ('ј', "j"),
    ('ѕ', "s"),
    // Cyrillic uppercase.
    ('А', "A"),
    ('В', "B"),
    ('Е', "E"),
    ('К', "K"),
    ('М', "M"),
    ('Н', "H"),
    ('О', "O"),
    ('Р', "P"),
    ('С', "C"),
    ('Т', "T"),
    ('У', "Y"),
    ('Х', "X"),
    ('І', "I"),
    ('Ј', "J"),
    ('Ѕ', "S"),
    // Greek lowercase.
    ('α', "a"),
    ('ι', "i"),
    ('κ', "k"),
    ('ν', "v"),
    ('ο', "o"),
    ('ρ', "p"),
    ('υ', "u"),
    ('χ', "x"),
    // Greek uppercase.
    ('Α', "A"),
    ('Β', "B"),
    ('Ε', "E"),
    ('Ζ', "Z"),
    ('Η', "H"),
    ('Ι', "I"),
    ('Κ', "K"),
    ('Μ', "M"),
    ('Ν', "N"),
    ('Ο', "O"),
    ('Ρ', "P"),
    ('Τ', "T"),
    ('Υ', "Y"),
    ('Χ', "X"),
];

pub(crate) fn confusable_latin(ch: char) -> Option<&'static str> {
    CONFUSABLE_TO_LATIN
        .iter()
        .find(|(c, _)| *c == ch)
        .map(|(_, l)| *l)
}

/// Per-char fold-then-match decisions for ONE alphanumeric token:
///
/// 1. NFKC identifier normalization: any ALPHABETIC char whose NFKC form
///    differs folds to that form unconditionally — fullwidth Latin and the
///    mathematical alphanumerics collapse to plain ASCII (UAX #31/TR39
///    identifier security). Non-alphabetic compatibility chars (½, ², …) are
///    deliberately untouched.
/// 2. Cross-script confusables fold when the token is MIXED-SCRIPT (carries
///    a post-NFKC ASCII letter) — the hard path — OR when the token is FULLY
///    FOLDABLE (every non-ASCII char either NFKC-folds to ASCII or is a
///    table entry). The fully-foldable path is flagged: the engine downgrades
///    any match inside it to CANDIDATE, the conservative tier for the rare
///    genuine-foreign-word collision. Genuine Russian/Greek words contain
///    letters with no Latin homoglyph (ж ш щ ю я д л …), never fully fold,
///    and stay byte-identical in the norm view.
///
/// Returns per-char replacements plus the fully-foldable flag, or None when
/// the token needs no change.
fn token_fold(token: &str) -> Option<(Vec<Option<String>>, bool)> {
    let nfkc: Vec<Option<String>> = token
        .chars()
        .map(|c| {
            if !c.is_ascii() && c.is_alphabetic() {
                let f: String = std::iter::once(c).nfkc().collect();
                if f != c.to_string() {
                    return Some(f);
                }
            }
            None
        })
        .collect();
    let witness = token.chars().zip(nfkc.iter()).any(|(c, f)| match f {
        Some(f) => !f.is_empty() && f.chars().all(|x| x.is_ascii_alphabetic()),
        None => c.is_ascii_alphabetic(),
    });
    let fully = !witness
        && !token.is_ascii()
        && token.chars().zip(nfkc.iter()).all(|(c, f)| {
            c.is_ascii() || matches!(f, Some(x) if x.is_ascii()) || confusable_latin(c).is_some()
        });
    let confusables_fold = witness || fully;
    let repl: Vec<Option<String>> = token
        .chars()
        .zip(nfkc)
        .map(|(c, f)| {
            if f.is_some() {
                return f;
            }
            if confusables_fold {
                if let Some(l) = confusable_latin(c) {
                    return Some(l.to_string());
                }
            }
            None
        })
        .collect();
    if repl.iter().all(Option::is_none) {
        None
    } else {
        Some((repl, fully))
    }
}

/// Every char replacement the token fold makes in `text`, token-wise, as
/// `(byte range of the char, replacement, fully_foldable)`.
fn fold_replacements(text: &str) -> Vec<(Range<usize>, String, bool)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < text.len() {
        let ch = text[i..].chars().next().unwrap_or('\u{FFFD}');
        if !ch.is_alphanumeric() {
            i += ch.len_utf8();
            continue;
        }
        let tlen: usize = text[i..]
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .map(|c| c.len_utf8())
            .sum();
        let token = &text[i..i + tlen];
        if !token.is_ascii() {
            if let Some((repl, fully)) = token_fold(token) {
                let mut at = i;
                for (c, r) in token.chars().zip(repl) {
                    let l = c.len_utf8();
                    if let Some(r) = r {
                        out.push((at..at + l, r, fully));
                    }
                    at += l;
                }
            }
        }
        i += tlen;
    }
    out
}

/// The token fold applied to a plain string (used by `render_key` so
/// the trigger-fidelity key matches the folded norm view).
fn fold_tokens_str(s: &str) -> String {
    let repls = fold_replacements(s);
    if repls.is_empty() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut at = 0usize;
    for (r, rep, _) in repls {
        out.push_str(&s[at..r.start]);
        out.push_str(&rep);
        at = r.end;
    }
    out.push_str(&s[at..]);
    out
}

/// Fold pass over a BUILT norm view: applies `fold_replacements` to the
/// fused norm text — so a token split across inline markup, HTML pieces, or
/// entity decodes is judged whole — rebuilding the segment table so every
/// span still maps to its exact source bytes. Fully-foldable replacements
/// carry `F_FULL_FOLD` on their segments; the engine downgrades matches
/// inside them to candidate tier.
pub(crate) fn fold_norm(old: NormView) -> NormView {
    let repls = fold_replacements(&old.text);
    if repls.is_empty() {
        return old;
    }
    let mut new = NormView {
        text: String::with_capacity(old.text.len()),
        segs: Vec::with_capacity(old.segs.len() + repls.len()),
        line_starts: Vec::with_capacity(old.line_starts.len()),
        block_starts: Vec::with_capacity(old.block_starts.len()),
        zero_width_removed: old.zero_width_removed.clone(),
    };
    // Old-norm-offset -> new-norm-offset checkpoints at segment starts;
    // line starts always fall on op boundaries, which are segment
    // boundaries (or end of text).
    let mut checkpoints: Vec<(usize, usize)> = Vec::new();
    let mut ri = 0usize;
    for seg in &old.segs {
        checkpoints.push((seg.norm.start, new.text.len()));
        // Replacements intersecting this segment.
        let mut cur = seg.norm.start;
        while ri < repls.len() && repls[ri].0.start < seg.norm.end {
            let (r, rep, fully) = &repls[ri];
            let fold_flags = seg.flags
                | if *fully {
                    crate::extract::F_FULL_FOLD
                } else {
                    0
                };
            match seg.kind {
                SegKind::Identity => {
                    // Identity prefix up to the folded char.
                    if cur < r.start {
                        let ns = new.text.len();
                        new.text.push_str(&old.text[cur..r.start]);
                        new.segs.push(Seg {
                            norm: ns..new.text.len(),
                            src: seg.src.start + (cur - seg.norm.start)
                                ..seg.src.start + (r.start - seg.norm.start),
                            kind: SegKind::Identity,
                            flags: seg.flags,
                        });
                    }
                    // The folded char maps its exact source bytes.
                    let ns = new.text.len();
                    new.text.push_str(rep);
                    new.segs.push(Seg {
                        norm: ns..new.text.len(),
                        src: seg.src.start + (r.start - seg.norm.start)
                            ..seg.src.start + (r.end - seg.norm.start),
                        kind: SegKind::Mapped,
                        flags: fold_flags,
                    });
                    cur = r.end;
                    ri += 1;
                }
                SegKind::Mapped => {
                    // Rewrite the whole mapped segment's content in one pass:
                    // consume every replacement inside it.
                    let ns = new.text.len();
                    let mut at = seg.norm.start;
                    let mut any_full = false;
                    while ri < repls.len() && repls[ri].0.start < seg.norm.end {
                        let (r2, rep2, fully2) = &repls[ri];
                        new.text.push_str(&old.text[at..r2.start]);
                        new.text.push_str(rep2);
                        any_full |= *fully2;
                        at = r2.end;
                        ri += 1;
                    }
                    new.text.push_str(&old.text[at..seg.norm.end]);
                    new.segs.push(Seg {
                        norm: ns..new.text.len(),
                        src: seg.src.clone(),
                        kind: SegKind::Mapped,
                        flags: seg.flags
                            | if any_full {
                                crate::extract::F_FULL_FOLD
                            } else {
                                0
                            },
                    });
                    cur = seg.norm.end;
                }
            }
        }
        // Unfolded tail (or the whole segment when nothing intersected).
        if cur < seg.norm.end || seg.norm.is_empty() {
            let ns = new.text.len();
            new.text.push_str(&old.text[cur..seg.norm.end]);
            new.segs.push(Seg {
                norm: ns..new.text.len(),
                src: match seg.kind {
                    SegKind::Identity => seg.src.start + (cur - seg.norm.start)..seg.src.end,
                    SegKind::Mapped => seg.src.clone(),
                },
                kind: seg.kind,
                flags: seg.flags,
            });
        }
    }
    checkpoints.push((old.text.len(), new.text.len()));
    let remap = |starts: &[usize], out: &mut Vec<usize>| {
        for ls in starts {
            let (o, n) = checkpoints
                .iter()
                .rev()
                .find(|(o, _)| o <= ls)
                .copied()
                .unwrap_or((0, 0));
            out.push(n + (ls - o));
        }
    };
    remap(&old.line_starts, &mut new.line_starts);
    remap(&old.block_starts, &mut new.block_starts);
    new
}

/// The enumerated-entity match at `at`, as `(byte_length, replacement)`.
/// Shared with `extract`'s numeric-reference anomaly scan so both sides agree
/// on which `&#…;` spellings the entity table already owns (`&#8212;` etc.).
pub(crate) fn entity_at(s: &str, at: usize) -> Option<(usize, &'static str)> {
    ENTITIES
        .iter()
        .find(|(e, _)| s[at..].starts_with(*e))
        .map(|(e, r)| (e.len(), *r))
}

/// Unicode `Cf` (format) ranges. A numeric reference to any of these is an
/// evasion signature (zero-width joiners, bidi controls, tags), never
/// legitimate typography — the enumeration is checked by classification, so
/// slight drift against a future Unicode version stays fail-closed only for
/// refs, never for literal text.
const FORMAT_RANGES: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

fn is_format_char(ch: char) -> bool {
    let c = ch as u32;
    FORMAT_RANGES.iter().any(|&(lo, hi)| lo <= c && c <= hi)
}

/// Unicode `Zs` space separators (NBSP, ogham space, en/em/thin spaces, …).
/// A reference to one renders as a visible space in both grammars, so it
/// folds to a plain space exactly as the entity table folds `&nbsp;` /
/// `&emsp;` / `&ensp;` / `&thinsp;` — layout, not evasion, and
/// `game&#xA0;changer` still reaches the two-word patterns.
fn is_space_separator(ch: char) -> bool {
    matches!(
        ch as u32,
        0xA0 | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000
    )
}

/// What a numeric character reference may decode INTO: not a C0/C1 control
/// (`char::is_control`), not a zero-width/format character, not a
/// default-ignorable (a ref to CGJ or a variation selector must
/// anomaly-flag, not decode into a char the literal path would then remove),
/// not whitespace other than a plain space (space separators were folded
/// before this check, so what reaches it is tab/LF/CR/FF — controls — and
/// the Zl/Zp line/paragraph separators). No legitimate document writes
/// `&#8203;` or `&#1;`; a reference to an invisible codepoint IS the
/// evasion signature.
fn is_ordinary_printable(ch: char) -> bool {
    ch == ' '
        || (!ch.is_control()
            && !ch.is_whitespace()
            && !is_format_char(ch)
            && !is_default_ignorable(ch))
}

/// Classification of a numeric character reference `&#DDD;` / `&#xHH;` at
/// `amp`. Decoding is pure arithmetic — no HTML5 named table — and bounded by
/// the CommonMark reference grammar (at most 7 decimal / 6 hex digits, `;`
/// required), so the decoder recognizes exactly what the markdown renderer
/// recognizes and can never fabricate a word the reader does not see
/// (over-decoding manufactures false positives).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumRef {
    /// In-bounds reference to an ordinary printable codepoint: decode it.
    /// A non-scalar value (surrogate, > U+10FFFF) decodes to U+FFFD exactly
    /// as CommonMark renders it.
    Decode(usize, char),
    /// In-bounds reference to a control/format/invisible codepoint — the
    /// evasion signature. The norm view elides it to
    /// U+FFFD; `extract` records the fail-closed SLOP-M005 anomaly.
    Suppress(usize),
    /// Valid digits terminated by `;` but outside the CommonMark bounds
    /// (overlong leading zeros). The renderer leaves it literal, so nothing
    /// can hide behind it in markdown; the norm view elides it to U+FFFD so
    /// its `;` never reads as prose punctuation, and
    /// HTML-derived text records the anomaly (a browser WOULD decode it).
    Overlong(usize),
    /// Not a numeric character reference.
    Literal,
}

pub(crate) fn classify_numeric_ref(s: &str, amp: usize) -> NumRef {
    let Some(body) = s[amp..].strip_prefix("&#") else {
        return NumRef::Literal;
    };
    let (digits, radix, prefix, cap) = match body.strip_prefix(['x', 'X']) {
        Some(hex) => (hex, 16u32, 3usize, 6usize),
        None => (body, 10u32, 2usize, 7usize),
    };
    let run = digits
        .bytes()
        .take_while(|b| match radix {
            16 => b.is_ascii_hexdigit(),
            _ => b.is_ascii_digit(),
        })
        .count();
    if run == 0 || digits.as_bytes().get(run) != Some(&b';') {
        return NumRef::Literal;
    }
    let len = prefix + run + 1;
    if run > cap {
        return NumRef::Overlong(len);
    }
    // Bounded digits always fit u32; from_u32 rejects surrogates and
    // > U+10FFFF, which CommonMark renders as the replacement character.
    let ch = char::from_u32(u32::from_str_radix(&digits[..run], radix).unwrap_or(u32::MAX))
        .unwrap_or('\u{FFFD}');
    if is_space_separator(ch) {
        return NumRef::Decode(len, ' ');
    }
    if is_ordinary_printable(ch) {
        NumRef::Decode(len, ch)
    } else {
        NumRef::Suppress(len)
    }
}

struct Builder<'a> {
    src: &'a str,
    out: NormView,
}

impl<'a> Builder<'a> {
    fn push_identity(&mut self, src_range: Range<usize>, flags: u8) {
        if src_range.start >= src_range.end {
            return;
        }
        let norm_start = self.out.text.len();
        self.out.text.push_str(&self.src[src_range.clone()]);
        // Coalesce with a previous adjacent identity segment.
        if let Some(last) = self.out.segs.last_mut() {
            if last.kind == SegKind::Identity
                && last.src.end == src_range.start
                && last.norm.end == norm_start
                && last.flags == flags
            {
                last.src.end = src_range.end;
                last.norm.end = self.out.text.len();
                return;
            }
        }
        self.out.segs.push(Seg {
            norm: norm_start..self.out.text.len(),
            src: src_range,
            kind: SegKind::Identity,
            flags,
        });
    }

    fn push_mapped(&mut self, src_range: Range<usize>, replacement: &str, flags: u8) {
        let norm_start = self.out.text.len();
        self.out.text.push_str(replacement);
        self.out.segs.push(Seg {
            norm: norm_start..self.out.text.len(),
            src: src_range,
            kind: SegKind::Mapped,
            flags,
        });
    }

    fn push_text(&mut self, src_range: Range<usize>, flags: u8) {
        let slice = &self.src[src_range.clone()];
        let base = src_range.start;
        let bytes = slice.as_bytes();
        let mut run_start = 0usize;
        let mut i = 0usize;
        while i < slice.len() {
            let b = bytes[i];
            if b == b'&' {
                if let Some((elen, rep)) = entity_at(slice, i) {
                    self.push_identity(base + run_start..base + i, flags);
                    self.push_mapped(base + i..base + i + elen, rep, flags);
                    i += elen;
                    run_start = i;
                    continue;
                }
                match classify_numeric_ref(slice, i) {
                    NumRef::Decode(len, ch) => {
                        self.push_identity(base + run_start..base + i, flags);
                        // The decoded char takes the same pipeline a literal
                        // char gets: classification already excludes
                        // zero-width and non-space whitespace, and NFC is
                        // applied here exactly as a literal non-ASCII run is
                        // normalized below (singletons like U+2126 compose).
                        let normalized: String = std::iter::once(ch).nfc().collect();
                        self.push_mapped(base + i..base + i + len, &normalized, flags);
                        i += len;
                        run_start = i;
                        continue;
                    }
                    NumRef::Suppress(len) | NumRef::Overlong(len) => {
                        // Fail closed: never inject the target codepoint
                        // (Suppress) or the raw `&#…;` bytes, whose `;` would
                        // read as prose punctuation. U+FFFD
                        // marks the spot without fabricating or fusing words;
                        // `extract`'s scan records the SLOP-M005 anomaly.
                        self.push_identity(base + run_start..base + i, flags);
                        self.push_mapped(base + i..base + i + len, "\u{FFFD}", flags);
                        i += len;
                        run_start = i;
                        continue;
                    }
                    NumRef::Literal => {
                        i += 1;
                        continue;
                    }
                }
            }
            if b == b'\\' && i + 1 < slice.len() && bytes[i + 1].is_ascii_punctuation() {
                self.push_identity(base + run_start..base + i, flags);
                let c = bytes[i + 1] as char;
                self.push_mapped(base + i..base + i + 2, &c.to_string(), flags);
                i += 2;
                run_start = i;
                continue;
            }
            if b < 0x80 {
                i += 1;
                continue;
            }
            // Non-ASCII: handle invisible removal, homoglyph folding, and
            // NFC in one run.
            let ch = slice[i..].chars().next().unwrap_or('\u{FFFD}');
            let ch_len = ch.len_utf8();
            if is_default_ignorable(ch) {
                self.push_identity(base + run_start..base + i, flags);
                self.push_mapped(base + i..base + i + ch_len, "", flags);
                self.out.zero_width_removed.push(base + i);
                i += ch_len;
                run_start = i;
                continue;
            }
            // Maximal run of non-ASCII chars needing no removal. Homoglyph
            // and NFKC folding happen in `fold_norm` AFTER the whole view is
            // built, so a token split across inline markup or HTML
            // pieces is judged FUSED, not per-op-slice.
            let na_start = i;
            let mut j = i;
            while j < slice.len() && bytes[j] >= 0x80 {
                let c = slice[j..].chars().next().unwrap_or('\u{FFFD}');
                if is_default_ignorable(c) {
                    break;
                }
                j += c.len_utf8();
            }
            let run = &slice[na_start..j];
            if is_nfc_quick(run.chars()) == IsNormalized::Yes {
                i = j;
                continue;
            }
            self.push_identity(base + run_start..base + na_start, flags);
            let normalized: String = run.nfc().collect();
            self.push_mapped(base + na_start..base + j, &normalized, flags);
            i = j;
            run_start = i;
        }
        self.push_identity(base + run_start..base + slice.len(), flags);
    }
}

/// Re-render an isolated fragment to a comparable text key: decode the
/// enumerated entities, resolve backslash escapes, drop zero-width characters,
/// strip HTML comments and real tags, NFC-normalize, fold whitespace runs to a
/// single space, and ASCII case-fold. Applied to BOTH the matched trigger (norm
/// text, where every transform is idempotent) and a finding's reported source
/// slice, it lets the trigger-fidelity invariant confirm the slice still
/// renders to the trigger without false-tripping on entities, escapes,
/// softbreak/whitespace folding, inline-tag fusion, or the whole-source
/// expansion of a Mapped segment (the caller compares by containment).
pub fn render_key(s: &str) -> String {
    // 1. Decode entities, resolve escapes, drop zero-width.
    let bytes = s.as_bytes();
    let mut decoded = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < s.len() {
        let b = bytes[i];
        if b == b'&' {
            if let Some((elen, rep)) = entity_at(s, i) {
                decoded.push_str(rep);
                i += elen;
                continue;
            }
            match classify_numeric_ref(s, i) {
                // Mirror push_text exactly so fidelity containment holds: decode
                // ordinary refs, elide suppressed/overlong refs to U+FFFD.
                NumRef::Decode(len, ch) => {
                    decoded.push(ch);
                    i += len;
                    continue;
                }
                NumRef::Suppress(len) | NumRef::Overlong(len) => {
                    decoded.push('\u{FFFD}');
                    i += len;
                    continue;
                }
                NumRef::Literal => {
                    decoded.push('&');
                    i += 1;
                    continue;
                }
            }
        }
        if b == b'\\' && i + 1 < s.len() && bytes[i + 1].is_ascii_punctuation() {
            decoded.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        let ch = s[i..].chars().next().unwrap_or('\u{FFFD}');
        if !is_default_ignorable(ch) {
            decoded.push(ch);
        }
        i += ch.len_utf8();
    }
    // 2. Strip HTML comments and real tags.
    let stripped = strip_html(&decoded);
    // 3. NFC, then the token fold (NFKC identifier normalization plus
    // cross-script homoglyphs) so the key matches the folded norm view, then
    // 4. fold whitespace runs and case-fold.
    let nfc: String = stripped.nfc().collect();
    let nfc = fold_tokens_str(&nfc);
    let mut out = String::with_capacity(nfc.len());
    let mut prev_ws = false;
    for ch in nfc.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.extend(ch.to_lowercase());
            prev_ws = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Remove HTML comments and real start/end tags (`<` immediately followed by an
/// ASCII letter, `/`, `!`, or `?` — the HTML5 tag-start rule). A `<` that is
/// literal text (e.g. `a < b`) is preserved, so prose angle brackets are not
/// eaten. Delimiters are ASCII, so byte indexing stays on char boundaries.
fn strip_html(s: &str) -> String {
    let bytes = s.as_bytes();
    let n = s.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    while i < n {
        if s[i..].starts_with("<!--") {
            match s[i + 4..].find("-->") {
                Some(e) => {
                    i = i + 4 + e + 3;
                    continue;
                }
                None => break,
            }
        }
        let b = bytes[i];
        let tag_start = b == b'<'
            && matches!(bytes.get(i + 1), Some(&c) if c.is_ascii_alphabetic() || c == b'/' || c == b'!' || c == b'?');
        if tag_start {
            match s[i..].find('>') {
                Some(e) => {
                    i += e + 1;
                    continue;
                }
                None => break,
            }
        }
        let ch = s[i..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub fn build_norm(src: &str, doc: &Doc) -> NormView {
    let mut b = Builder {
        src,
        out: NormView::default(),
    };
    for op in &doc.ops {
        match op {
            NormOp::Block => {
                if !b.out.text.is_empty() && !b.out.text.ends_with('\n') {
                    let at = b.out.segs.last().map(|s| s.src.end).unwrap_or(0);
                    b.push_mapped(at..at, "\n", 0);
                }
                b.out.line_starts.push(b.out.text.len());
                b.out.block_starts.push(b.out.text.len());
            }
            NormOp::Text { range, flags } => {
                b.push_text(range.clone(), *flags);
            }
            NormOp::TextOwned {
                range,
                content,
                flags,
            } => {
                b.push_mapped(range.clone(), content, *flags);
            }
            NormOp::Break { range, hard, flags } => {
                b.push_mapped(range.clone(), if *hard { "\n" } else { " " }, *flags);
                // Only a hard break starts a line. A soft break is where the
                // author's editor wrapped a paragraph, and the reader never
                // sees it, so a word after one is mid-sentence and no rule
                // anchored to a line start may fire there.
                if *hard {
                    b.out.line_starts.push(b.out.text.len());
                }
            }
            // An excluded inline region (inline code, autolink) whose
            // rendered content visibly interrupts the prose. U+FFFD is not a
            // word character, not whitespace, and not a line break, so
            // flanking runs neither fuse into a word nor assemble a phrase
            // nor gain a block start.
            NormOp::Barrier { range, flags } => {
                b.push_mapped(range.clone(), "\u{FFFD}", *flags);
            }
        }
    }
    fold_norm(b.out)
}

impl NormView {
    /// Map a norm byte range to a source byte range. Identity segments map
    /// linearly; mapped segments expand to their whole source range.
    pub fn to_source(&self, norm_range: Range<usize>) -> Option<Range<usize>> {
        if norm_range.start >= norm_range.end || self.segs.is_empty() {
            return None;
        }
        let first = self.seg_at(norm_range.start)?;
        let last = self.seg_at(norm_range.end - 1)?;
        let fs = &self.segs[first];
        let ls = &self.segs[last];
        let start = match fs.kind {
            SegKind::Identity => fs.src.start + (norm_range.start - fs.norm.start),
            SegKind::Mapped => fs.src.start,
        };
        let end = match ls.kind {
            SegKind::Identity => ls.src.start + (norm_range.end - ls.norm.start),
            SegKind::Mapped => ls.src.end,
        };
        if end < start {
            return None;
        }
        Some(start..end)
    }

    /// Concatenate the norm text of every segment intersecting a SOURCE range.
    /// Excluded bytes (code fences, inline code, HTML markup, link URLs,
    /// autolinks) carry no segment and so contribute nothing, which is exactly
    /// how the norm view renders them. This reconstructs the norm text a source
    /// span carries, letting the trigger-fidelity check confirm a reported
    /// span still renders to its trigger even when `to_source` legitimately
    /// widened it across excluded bytes — without re-implementing the pipeline.
    pub fn source_span_norm_text(&self, src: &Range<usize>) -> String {
        let mut out = String::new();
        // Segments are emitted in source order, so src.start and src.end are
        // both non-decreasing across the table: binary-search the first
        // segment that can intersect and stop at the first that starts past
        // the span, exactly as `seg_at` resolves norm offsets. This is called
        // once per hit, and a linear scan here made assembly O(hits x segs).
        let first = self.segs.partition_point(|seg| seg.src.end <= src.start);
        for seg in &self.segs[first..] {
            if seg.src.start >= src.end {
                break;
            }
            if src.start >= seg.src.end {
                continue;
            }
            match seg.kind {
                // Identity: norm bytes equal source bytes, so borrow ONLY the
                // sub-slice the source span actually overlaps. Appending the
                // whole segment let a span that touched one byte of a
                // trigger-bearing paragraph inherit the entire paragraph's
                // norm text and spuriously pass the fidelity check (the
                // displaced-span class).
                SegKind::Identity => {
                    let lo = seg.src.start.max(src.start);
                    let hi = seg.src.end.min(src.end);
                    let noff = seg.norm.start + (lo - seg.src.start);
                    let nend = seg.norm.start + (hi - seg.src.start);
                    out.push_str(&self.text[noff..nend]);
                }
                // Mapped: norm and source differ in length and a match
                // touching any part maps to the whole source range by design
                // (entities, escapes, softbreaks, owned content), so it
                // legitimately contributes its whole norm text.
                SegKind::Mapped => out.push_str(&self.text[seg.norm.clone()]),
            }
        }
        out
    }

    /// Flags of the segment containing a norm offset.
    pub fn flags_at(&self, norm_offset: usize) -> u8 {
        self.seg_at(norm_offset)
            .map(|i| self.segs[i].flags)
            .unwrap_or(0)
    }

    /// Any covering segment carries the given flag bit.
    pub fn span_has_flag(&self, norm_range: &Range<usize>, bit: u8) -> bool {
        let Some(first) = self.seg_at(norm_range.start) else {
            return false;
        };
        let Some(last) = self.seg_at(norm_range.end.saturating_sub(1)) else {
            return false;
        };
        self.segs[first..=last].iter().any(|s| s.flags & bit != 0)
    }

    /// All covering segments carry the quoted flag.
    pub fn all_quoted(&self, norm_range: &Range<usize>) -> bool {
        let Some(first) = self.seg_at(norm_range.start) else {
            return false;
        };
        let Some(last) = self.seg_at(norm_range.end.saturating_sub(1)) else {
            return false;
        };
        self.segs[first..=last]
            .iter()
            .filter(|s| s.norm.start < s.norm.end)
            .all(|s| s.flags & crate::extract::F_QUOTED != 0)
    }

    fn seg_at(&self, norm_offset: usize) -> Option<usize> {
        // Binary search over non-empty norm ranges; empty segments (removals)
        // never contain an offset.
        let mut lo = 0usize;
        let mut hi = self.segs.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let s = &self.segs[mid];
            if norm_offset < s.norm.start {
                hi = mid;
            } else if norm_offset >= s.norm.end {
                lo = mid + 1;
            } else {
                return Some(mid);
            }
        }
        // Fall back to the nearest previous non-empty segment.
        (0..self.segs.len())
            .rev()
            .find(|&i| self.segs[i].norm.start <= norm_offset && !self.segs[i].norm.is_empty())
    }

    /// True when the position begins a block or line, or follows terminal
    /// punctuation plus whitespace.
    pub fn is_block_start(&self, norm_offset: usize) -> bool {
        if norm_offset == 0 {
            return true;
        }
        let before = &self.text[..norm_offset];
        let trimmed = before.trim_end_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed.ends_with('\n') {
            return true;
        }
        if trimmed.ends_with(['.', '!', '?', ':']) && trimmed.len() < before.len() {
            return true;
        }
        // A recorded line start with only whitespace between it and here.
        if let Some(&ls) = self.line_starts.iter().rev().find(|&&ls| ls <= norm_offset) {
            if self.text[ls..norm_offset]
                .chars()
                .all(|c| c == ' ' || c == '\t' || c == '\n')
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{Doc, NormOp};

    fn norm(src: &str, ops: Vec<NormOp>) -> NormView {
        let doc = Doc {
            ops,
            ..Default::default()
        };
        build_norm(src, &doc)
    }

    // Numeric character references are
    // CLASSIFIED — ordinary printables decode, invisibles suppress, overlong
    // forms elide, everything else stays literal.
    #[test]
    fn numeric_ref_classification_is_arithmetic_and_fail_closed() {
        use NumRef::*;
        assert_eq!(classify_numeric_ref("&#100;", 0), Decode(6, 'd'));
        assert_eq!(classify_numeric_ref("&#x65;", 0), Decode(6, 'e'));
        assert_eq!(classify_numeric_ref("&#X65;", 0), Decode(6, 'e'));
        assert_eq!(classify_numeric_ref("&#32;", 0), Decode(5, ' '));
        assert_eq!(classify_numeric_ref("xy&#100;", 2), Decode(6, 'd'));
        // Ordinary typography decodes (guardrail: © and é are not anomalies).
        assert_eq!(classify_numeric_ref("&#169;", 0), Decode(6, '\u{A9}'));
        assert_eq!(classify_numeric_ref("&#233;", 0), Decode(6, '\u{E9}'));
        // Space separators fold to a plain space like &nbsp;/&emsp; do.
        assert_eq!(classify_numeric_ref("&#xA0;", 0), Decode(6, ' '));
        assert_eq!(classify_numeric_ref("&#8195;", 0), Decode(7, ' ')); // emsp

        // The ZWSP neighbor one codepoint past the Zs range stays suppressed.
        assert_eq!(classify_numeric_ref("&#x200B;", 0), Suppress(8));
        // Invisible/control targets are the evasion signature: suppressed.
        assert_eq!(classify_numeric_ref("&#8203;", 0), Suppress(7)); // ZWSP
        assert_eq!(classify_numeric_ref("&#9;", 0), Suppress(4)); // tab
        assert_eq!(classify_numeric_ref("&#1;", 0), Suppress(4)); // C0
        assert_eq!(classify_numeric_ref("&#151;", 0), Suppress(6)); // C1 U+0097
        assert_eq!(classify_numeric_ref("&#xFEFF;", 0), Suppress(8));
        assert_eq!(classify_numeric_ref("&#x202E;", 0), Suppress(8)); // RLO

        // Default-ignorables OUTSIDE Cf also suppress — a ref to
        // CGJ (Mn) or a variation selector must not decode into a char the
        // literal path would then silently remove.
        assert_eq!(classify_numeric_ref("&#847;", 0), Suppress(6)); // CGJ
        assert_eq!(classify_numeric_ref("&#xFE0F;", 0), Suppress(8)); // VS16
        assert_eq!(classify_numeric_ref("&#173;", 0), Suppress(6)); // SHY

        // CommonMark bounds: >7 decimal / >6 hex digits is overlong, elided.
        assert_eq!(classify_numeric_ref("&#x0000064;", 0), Overlong(11));
        assert_eq!(classify_numeric_ref("&#0000000169;", 0), Overlong(13));
        assert_eq!(classify_numeric_ref("&#99999999;", 0), Overlong(11));
        // Malformed or named refs stay literal.
        assert_eq!(classify_numeric_ref("&#", 0), Literal);
        assert_eq!(classify_numeric_ref("&#;", 0), Literal);
        assert_eq!(classify_numeric_ref("&#zz;", 0), Literal);
        assert_eq!(classify_numeric_ref("&#xZZ;", 0), Literal);
        assert_eq!(classify_numeric_ref("&amp;", 0), Literal);
        assert_eq!(classify_numeric_ref("&#100elve", 0), Literal); // no `;`
                                                                   // Non-scalar values decode to U+FFFD exactly as CommonMark renders.
        assert_eq!(classify_numeric_ref("&#xD800;", 0), Decode(8, '\u{FFFD}'));
    }

    #[test]
    fn numeric_ref_flows_through_norm_view_and_render_key() {
        // Reference-spelled evasions render to the real word in the norm view.
        let nv = norm(
            "&#100;elve",
            vec![NormOp::Text {
                range: 0..10,
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "delve");
        // render_key decodes them too, so the fidelity check sees the same
        // word on the raw slice.
        assert_eq!(render_key("&#100;elve"), "delve");
        assert_eq!(render_key("d&#x65;lve"), "delve");
        assert_eq!(render_key("game&#32;changer"), "game changer");
        // A bare/malformed ref is left literal by render_key.
        assert_eq!(render_key("a&#b"), "a&#b");
    }

    // source_span_norm_text clips Identity segments to the actual
    // source overlap so a displaced span cannot borrow a whole paragraph.
    #[test]
    fn source_span_norm_text_clips_identity_segments() {
        let nv = norm(
            "delve parser",
            vec![NormOp::Text {
                range: 0..12,
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "delve parser");
        // A one-byte source span borrows ONLY that byte's norm text.
        assert_eq!(nv.source_span_norm_text(&(6..7)), "p");
        // So a span displaced onto "parser" cannot inherit the paragraph's
        // "delve" (the pre-fix bug returned the whole segment text).
        assert!(!nv.source_span_norm_text(&(6..12)).contains("delve"));
    }

    // The fold pass rewrites the segment table; a folded char must
    // still map to its exact source bytes, and fully-folded tokens carry
    // the candidate flag.
    #[test]
    fn fold_pass_preserves_source_mapping_and_flags() {
        // "dеlve": the Cyrillic е occupies source bytes 1..3.
        let nv = norm(
            "d\u{0435}lve",
            vec![NormOp::Text {
                range: 0..6,
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "delve");
        // The folded char's norm byte (offset 1) maps to source 1..3.
        assert_eq!(nv.to_source(1..2), Some(1..3));
        assert!(!nv.span_has_flag(&(0..5), crate::extract::F_FULL_FOLD));
        // Fully-foldable token: folds and carries the candidate flag.
        let src = "моѕаіс";
        let nv = norm(
            src,
            vec![NormOp::Text {
                range: 0..src.len(),
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "mosaic");
        assert!(nv.span_has_flag(&(0..6), crate::extract::F_FULL_FOLD));
        // NFKC identifier fold: fullwidth collapses, hard (no full-fold flag).
        let src = "\u{FF4D}\u{FF4F}\u{FF53}\u{FF41}\u{FF49}\u{FF43}";
        let nv = norm(
            src,
            vec![NormOp::Text {
                range: 0..src.len(),
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "mosaic");
        assert!(!nv.span_has_flag(&(0..6), crate::extract::F_FULL_FOLD));
        assert_eq!(
            render_key("\u{FF4D}\u{FF4F}\u{FF53}\u{FF41}\u{FF49}\u{FF43}"),
            "mosaic"
        );
        assert_eq!(render_key("моѕаіс"), "mosaic");
        // Non-identifier compatibility chars stay put.
        let nv = norm(
            "3½ cups",
            vec![NormOp::Text {
                range: 0..8,
                flags: 0,
            }],
        );
        assert!(nv.text.contains('½'));
    }

    // Cross-script homoglyphs fold to Latin inside mixed-script tokens,
    // in both the norm view and render_key (fidelity containment).
    #[test]
    fn homoglyphs_fold_only_in_mixed_script_tokens() {
        let nv = norm(
            "d\u{0435}lve",
            vec![NormOp::Text {
                range: 0..6,
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "delve");
        assert_eq!(render_key("d\u{0435}lve"), "delve");
        // A pure-Cyrillic word never folds (delve spelled fully Cyrillic).
        let src = "делве";
        let nv = norm(
            src,
            vec![NormOp::Text {
                range: 0..src.len(),
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "делве");
        assert_eq!(render_key("делве"), "делве");
        // Latin-script accents are not cross-script confusables.
        let nv = norm(
            "café",
            vec![NormOp::Text {
                range: 0..5,
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "café");
    }

    #[test]
    fn source_span_norm_text_keeps_whole_mapped_segment() {
        // "a&mdash;b" → Identity "a", Mapped(em-dash), Identity "b".
        let nv = norm(
            "a&mdash;b",
            vec![NormOp::Text {
                range: 0..9,
                flags: 0,
            }],
        );
        assert_eq!(nv.text, "a\u{2014}b");
        // A span touching one byte inside the &mdash; run still yields the whole
        // mapped char — Mapped keeps whole-range semantics by design.
        assert!(nv.source_span_norm_text(&(3..4)).contains('\u{2014}'));
    }
}
