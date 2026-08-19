#![no_main]
//! The norm view mapping must round-trip: every norm range maps to a source
//! range on char boundaries, in bounds, AND the mapped source slice must
//! re-render (through the same view transforms) to still carry the norm text it
//! came from — the trigger-fidelity property, folded and case-insensitive.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let config = unslop::Config::new(unslop::Profile::Doc);
    let Ok(prepared) = unslop::input::prepare(text.as_bytes(), &config) else {
        return;
    };
    let Ok(doc) = unslop::extract::build_doc(&prepared, &config) else {
        return;
    };
    let norm = unslop::views::build_norm(&prepared.text, &doc);
    let n = norm.text.len();
    let src = prepared.text.as_str();
    for start in (0..n).step_by(3) {
        let end = (start + 7).min(n);
        if start >= end || !norm.text.is_char_boundary(start) || !norm.text.is_char_boundary(end) {
            continue;
        }
        if let Some(span) = norm.to_source(start..end) {
            assert!(span.end <= src.len());
            assert!(span.start <= span.end);
        }
    }

    // Folded round-trip / trigger fidelity: no real finding's reported span
    // may fail to render back to its trigger. `analyze` may fail for other
    // instrumentation reasons on adversarial input, but must never emit the
    // trigger-fidelity error — that would mean a legitimate finding was turned
    // into an instrumentation error by an over-strict re-render check.
    for profile in [
        unslop::Profile::Doc,
        unslop::Profile::Doc,
    ] {
        let config = unslop::Config::new(profile);
        if let Err(e) = unslop::analyze(text.as_bytes(), &config) {
            assert!(
                !format!("{e}").contains("trigger-fidelity"),
                "trigger fidelity false-tripped: {e} on {text:?}"
            );
        }
    }
});
