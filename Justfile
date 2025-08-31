# Use bash with strict flags
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# -------------------------------------------------------------------
# Paths & tooling
# -------------------------------------------------------------------
SCHEMAS_DIR := "tests/schemas"

# Rust binary path
RUST_BIN := "target/debug/git-activity-report"

# -------------------------------------------------------------------
# Help
# -------------------------------------------------------------------
_help:
  @echo "Recipes:"
  @echo "  build         # cargo build"
  @echo "  test          # cargo test"
  @echo "  fmt           # cargo fmt --check"
  @echo "  clippy        # cargo clippy -D warnings"
  @echo "  help          # print Rust CLI --help (builds first)"
  @echo "  run-simple    # sample run of Rust CLI (prints normalized config)"
  @echo "  run-full      # sample full-mode run (config print for now)"
  @echo "  test          # run tests (nextest + coverage + schema validation)"

doctor:
  set +e
  echo "Rust toolchain:"; rustup show || true

# -------------------------------------------------------------------
# Rust workflow
# -------------------------------------------------------------------

build:
  cargo build

test:
  NEXTEST_EXPERIMENTAL_SETUP_SCRIPTS=1 cargo llvm-cov nextest

fmt:
  cargo fmt --all -- --check

clippy:
  cargo clippy -- -D warnings

# Sample: print normalized config for a simple window
run-simple: build
  {{RUST_BIN}} --simple --for "last week" --repo .

# Sample: print normalized config for a full window
run-full: build
  {{RUST_BIN}} --full --month 2025-08 --split-out .tmp/out

## Legacy validations and fixture flows removed in favor of Rust-side schema tests.

# Spacing audit (reports potential misses; manual fixes per SPACING.md)
audit-spacing:
  bash scripts/spacing-audit.sh normal src

audit-spacing-strict:
  bash scripts/spacing-audit.sh strict src
