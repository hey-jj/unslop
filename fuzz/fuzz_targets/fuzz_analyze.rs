#![no_main]
//! The extractor and full pipeline must never panic on arbitrary bytes, and
//! every emitted span must hold the emit-time invariant, which analyze
//! enforces internally.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for profile in unslop::Profile::ALL {
        let config = unslop::Config::new(profile);
        let _ = unslop::analyze(data, &config);
    }
});
