# Local quality gate. `just` runs what CI runs (minus the heavy build jobs).

default: check

# Everything: format, lint, test, supply-chain.
check: fmt clippy test deny audit

fmt:
    cargo fmt --check

clippy:
    cargo clippy --all-targets --locked -- -D warnings

test:
    cargo test --locked

deny:
    cargo deny check

audit:
    cargo audit

# Coverage (requires cargo-llvm-cov). Enforced at 100% on the core lib in CI
# once the workspace split lands (Phase 3).
cov:
    cargo llvm-cov --summary-only
