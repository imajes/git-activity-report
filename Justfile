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
  @echo "  validate-all                 # Validate all schemas/fixtures"
  @echo "  validate-simple              # simple schema -> fixture"
  @echo "  validate-range               # full range manifest"
  @echo "  validate-top                 # full top manifest"
  @echo "  validate-commit-shards       # validate all commit shard fixtures"
  @echo "  validate schema data         # validate an arbitrary pair"
  @echo "  fmt-fixtures                 # format fixtures with jq"
  @echo "  doctor                       # show ajv + rust toolchain status"
  @echo "  build-fixtures               # synthesize tiny repo & refresh fixtures (if script exists)"
  @echo "  rust-build                   # cargo build"
  @echo "  rust-test                    # cargo test"
  @echo "  rust-fmt                     # cargo fmt --check"
  @echo "  rust-clippy                  # cargo clippy -D warnings"
  @echo "  rust-help                    # print Rust CLI --help (builds first)"
  @echo "  rust-run-simple              # sample run of Rust CLI (prints normalized config)"
  @echo "  rust-run-full                # sample full-mode run (config print for now)"
  @echo "  test                         # run help snapshots + golden diff"

# -------------------------------------------------------------------
# Validation (JSON Schema / fixtures)
# -------------------------------------------------------------------

# Generic validator: pass logical schema key and explicit data path
# Example: just validate simple tests/fixtures/git-activity-report.simple.fixture.json
validate schema data:
  {{AJV}} {{AJVFLAGS}} validate \
    -s {{SCHEMAS_DIR}}/git-activity-report.{{schema}}.schema.json \
    -d {{data}}
  @echo "✔ {{data}} ✓"

# Simple (current schema)
validate-simple:
  {{AJV}} {{AJVFLAGS}} validate \
    -s {{SCHEMAS_DIR}}/git-activity-report.simple.schema.json \
    -d {{FIXTURES_DIR}}/git-activity-report.simple.fixture.json
  @echo "✔ simple OK"

# Full range manifest
validate-range:
  {{AJV}} {{AJVFLAGS}} validate \
    -s {{SCHEMAS_DIR}}/git-activity-report.full.range.schema.json \
    -d {{FIXTURES_DIR}}/manifest-2025-08.json
  @echo "✔ range manifest OK"

# Full top manifest
validate-top:
  {{AJV}} {{AJVFLAGS}} validate \
    -s {{SCHEMAS_DIR}}/git-activity-report.full.top.schema.json \
    -d {{FIXTURES_DIR}}/manifest.json
  @echo "✔ top manifest OK"

# Validate all commit shards that look like YYYY.MM.DD-HH.MM-<sha>.json
validate-commit-shards:
  # Use ajv's built-in glob expansion by quoting the pattern
  {{AJV}} {{AJVFLAGS}} validate \
    -s {{SCHEMAS_DIR}}/git-activity-report.commit.schema.json \
    -d "{{FIXTURES_DIR}}/[0-9][0-9][0-9][0-9].[0-9][0-9].[0-9][0-9]-[0-9][0-9].[0-9][0-9]-*.json"
  @echo "✔ commit shards OK"

# Everything
validate-all: validate-simple validate-commit-shards validate-range validate-top
  @echo "---------------------------------------------"
  @echo "All validations passed ✓"
  @echo "Schemas:  $$(ls -1 {{SCHEMAS_DIR}}/*.json | wc -l)   Fixtures: $$(ls -1 {{FIXTURES_DIR}}/*.json | wc -l)"
  @echo "---------------------------------------------"

fmt-fixtures:
  for f in {{FIXTURES_DIR}}/*.json; do \
    tmp="$${f}.tmp"; {{JQ}} . "$$f" > "$$tmp" && mv "$$tmp" "$$f"; \
    echo "fmt: $$f"; \
  done

# If you added tests/scripts/make-fixture-repo.sh
build-fixtures:
  if [ -x tests/scripts/make-fixture-repo.sh ]; then \
    bash tests/scripts/make-fixture-repo.sh; \
  else \
    echo "No tests/scripts/make-fixture-repo.sh found (skipping)"; \
  fi

doctor:
  set +e
  echo "AJV CLI:"; {{AJV}} help >/dev/null 2>&1 && echo "ajv OK" || echo "ajv NOT OK"
  echo "Testing draft engine ..."; {{AJV}} {{AJVFLAGS}} help >/dev/null 2>&1 && echo "draft2020 OK" || echo "draft2020 NOT OK"
  if ! command -v {{JQ}} >/dev/null 2>&1; then echo "jq not found — please install jq"; exit 1; else echo "jq: $$(jq --version)"; fi
  echo "Rust toolchain:"; rustup show || true

# -------------------------------------------------------------------
# Rust workflow
# -------------------------------------------------------------------

rust-build:
  cargo build

rust-test:
  cargo test

rust-fmt:
  cargo fmt --all -- --check

rust-clippy:
  cargo clippy -- -D warnings

rust-help: rust-build
  {{RUST_BIN}} --help | sed -e 's#\\(.\\)/.*git-activity-report#git-activity-report#g'

# Sample: print normalized config for a simple window
rust-run-simple: rust-build
  {{RUST_BIN}} --simple --for "last week" --repo . | {{JQ}} .

# Sample: print normalized config for a full window
rust-run-full: rust-build
  {{RUST_BIN}} --full --month 2025-08 --split-out .tmp/out | {{JQ}} .

# High-level test wrapper delegating to tests/Justfile
test: build-fixtures rust-build
  just -f tests/Justfile help-py
  just -f tests/Justfile help-rs
  just -f tests/Justfile version-snap || true
  just -f tests/Justfile golden
  just -f tests/Justfile golden-rs
  just -f tests/Justfile validate-rs-full

# Spacing audit (reports potential misses; manual fixes per SPACING.md)
audit-spacing:
  bash scripts/spacing-audit.sh normal src

audit-spacing-strict:
  bash scripts/spacing-audit.sh strict src
