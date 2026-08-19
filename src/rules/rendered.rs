//! rendered-view family dispatch: SLOP-Y001 render-invisible content and
//! SLOP-Y002 render divergence, over the locally computed rendered view.

use crate::engine::{CompiledPolicy, Hit};
use crate::extract::Doc;
use crate::input::Prepared;
use crate::Config;

pub const HANDLED: &[&str] = &["SLOP-Y001", "SLOP-Y002"];

pub fn evaluate(
    cp: &CompiledPolicy,
    prepared: &Prepared,
    doc: &Doc,
    config: &Config,
    hits: &mut Vec<Hit>,
) {
    if let Some(idx) = super::active(cp, config, "SLOP-Y001") {
        crate::render::render_invisible(prepared, doc, config, idx, hits);
    }
    if let Some(idx) = super::active(cp, config, "SLOP-Y002") {
        crate::render::render_divergence(prepared, doc, config, idx, hits);
    }
}
