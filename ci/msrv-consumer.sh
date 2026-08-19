#!/usr/bin/env bash
# MSRV gate for the declared rust-version (1.85).
#
# The shipped Cargo.lock is inert for dependents: a downstream user's cargo
# resolves the dependency graph fresh from the index. So this gate builds a
# real external consumer crate that depends on unslop by path, with NO
# lockfile carried over, on a 1.85 toolchain, exactly what a downstream
# user at the MSRV floor experiences. It then builds unslop itself at 1.85
# to prove the crate's own code compiles at the declared floor.
#
# Run it with a 1.85.x toolchain active (e.g. `rustup run 1.85.0 ci/msrv-consumer.sh`).
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "$0")/.." && pwd)"

RUSTC_VERSION="$(rustc --version)"
echo "active toolchain: $RUSTC_VERSION"
case "$RUSTC_VERSION" in
*" 1.85."*) ;;
*)
    echo "error: this gate must run on Rust 1.85.x (the declared MSRV); got: $RUSTC_VERSION" >&2
    exit 1
    ;;
esac

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
CONSUMER="$WORKDIR/msrv-consumer"
mkdir -p "$CONSUMER/src"

cat >"$CONSUMER/Cargo.toml" <<EOF
[package]
name = "msrv-consumer"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
unslop = { path = "$CRATE_DIR" }
EOF

cat >"$CONSUMER/src/main.rs" <<'EOF'
fn main() {
    let config = unslop::Config::new(unslop::Profile::Doc);
    let report = unslop::analyze(b"MSRV consumer smoke input.\n", &config)
        .expect("analyze must succeed on plain input");
    println!(
        "unslop MSRV consumer ok: {} findings, policy {}",
        report.findings.len(),
        unslop::policy_digest()
    );
}
EOF

# Deliberately no lockfile in $CONSUMER: cargo resolves fresh from the index,
# as any dependent would. A transitive dep whose current version needs >1.85
# fails this build even though unslop's own pinned lockfile builds fine.
echo "== external consumer: fresh-resolution build at 1.85 =="
cargo build --manifest-path "$CONSUMER/Cargo.toml"

echo "== external consumer: run =="
cargo run --quiet --manifest-path "$CONSUMER/Cargo.toml"

echo "== unslop itself: build at 1.85 (own lockfile) =="
cargo build --manifest-path "$CRATE_DIR/Cargo.toml"

echo "MSRV gate passed at: $RUSTC_VERSION"
