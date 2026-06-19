# Local quality gate. `just` runs what CI runs (minus the heavy build jobs).

default: check

# Everything: format, lint, docs, test, supply-chain.
check: fmt clippy doc test deny audit

fmt:
    cargo fmt --check

clippy:
    cargo clippy --workspace --all-targets --locked -- -D warnings

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items

test:
    cargo test --workspace --locked

deny:
    cargo deny check

audit:
    cargo audit

# Coverage (requires cargo-llvm-cov): pure-logic modules + the privileged core
# contract must stay 100%; the GTK shell and I/O-mixed backend are excluded.
cov:
    cargo llvm-cov --workspace --fail-under-lines 100 --ignore-filename-regex 'src/(ui|main\.rs|config\.rs|backend)'

# Full coverage summary across everything (not gated).
cov-all:
    cargo llvm-cov --workspace --summary-only
