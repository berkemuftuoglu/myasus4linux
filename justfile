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

# Coverage (requires cargo-llvm-cov): pure-logic modules must stay 100%, the
# I/O-mixed backend is held to a floor, the GTK shell is excluded.
cov:
    cargo llvm-cov --no-report
    cargo llvm-cov report --fail-under-lines 100 --ignore-filename-regex 'src/(ui|main\.rs|config\.rs|backend)'
    cargo llvm-cov report --fail-under-lines 60 --ignore-filename-regex 'src/(ui|main\.rs|config\.rs)'
    cargo llvm-cov report --summary-only --ignore-filename-regex 'src/(ui|main\.rs|config\.rs)'
