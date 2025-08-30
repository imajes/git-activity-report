# Use bash with strict flags
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# -------------------------------------------------------------------
# Paths & tooling
# -------------------------------------------------------------------
SCHEMAS_DIR := "tests/schemas"
FIXTURES_DIR := "tests/fixtures"

# Ajv CLI (Draft 2020-12) — prefer local `ajv`
AJV      := "ajv"
AJVFLAGS := "--spec=draft2020"

# Pretty printer (required for development)
JQ := "jq"

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
  @echo "  test          # run help snapshots + golden diff"

doctor:
  set +e
  echo "AJV CLI:"; {{AJV}} help >/dev/null 2>&1 && echo "ajv OK" || echo "ajv NOT OK"
  echo "Testing draft engine ..."; {{AJV}} {{AJVFLAGS}} help >/dev/null 2>&1 && echo "draft2020 OK" || echo "draft2020 NOT OK"
  if ! command -v {{JQ}} >/dev/null 2>&1; then echo "jq not found — please install jq"; exit 1; else echo "jq: $$(jq --version)"; fi
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
  {{RUST_BIN}} --simple --for "last week" --repo . | {{JQ}} .

# Sample: print normalized config for a full window
run-full: build
  {{RUST_BIN}} --full --month 2025-08 --split-out .tmp/out | {{JQ}} .

# # High-level test wrapper delegating to tests/Justfile
# test: build-fixtures build
#   just -f tests/Justfile help-py
#   just -f tests/Justfile help-rs
#   just -f tests/Justfile golden-rs
#   just -f tests/Justfile validate-rs-full

# Spacing audit (reports potential misses; manual fixes per SPACING.md)
audit-spacing:
  bash scripts/spacing-audit.sh normal src

audit-spacing-strict:
  bash scripts/spacing-audit.sh strict src
