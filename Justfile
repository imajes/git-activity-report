# Use bash with strict flags
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Project root and paths
ROOT        := justfile_directory()
SCHEMAS_DIR := "tests/schemas"
FIXTURES_DIR:= "tests/fixtures"

# AJV command: override with `just AJV=ajv validate-all` if you have it globally
# Otherwise we'll default to an npx one-shot for zero-setup.
AJV := "npx --yes ajv-cli --spec=draft2020"
JQ := "jq"

# -------------------------------------------------------------------
# Help
# -------------------------------------------------------------------
_help:
  @echo "Available Recipes:"
  @echo "  validate-all                      # Validate all schemas against fixtures"
  @echo "  validate-simple                   # Validate simple -> simple.fixture.json"
  @echo "  validate-range                    # Validate full range manifest"
  @echo "  validate-top                      # Validate full top manifest"
  @echo "  validate-commit-shards            # Validate the two commit shard fixtures"
  @echo "  validate schema data              # Validate an arbitrary pair"
  @echo "  fmt-fixtures                      # Pretty-print fixtures in-place (requires jq)"
  @echo "  show-env                          # Show resolved paths and tools"

# -------------------------------------------------------------------
# One-offs
# -------------------------------------------------------------------

# Generic validator: pass logical schema key and explicit data path
# Usage: just validate simple tests/fixtures/git-activity-report.simple.fixture.json
validate schema data:
  {{AJV}} validate -s {{SCHEMAS_DIR}}/git-activity-report.{{schema}}.schema.json \
                   -d {{data}}
  @echo "✔ Validated {{data}} against {{SCHEMAS_DIR}}/git-activity-report.{{schema}}.schema.json"

# Simple (current): timestamps include author_local/commit_local/timezone
validate-simple:
  {{AJV}} validate -s {{SCHEMAS_DIR}}/git-activity-report.simple.schema.json \
                   -d {{FIXTURES_DIR}}/git-activity-report.simple.fixture.json
  @echo "✔ simple OK"

# Range manifest (full mode)
validate-range:
  {{AJV}} validate -s {{SCHEMAS_DIR}}/git-activity-report.full.range.schema.json \
                   -d {{FIXTURES_DIR}}/manifest-2025-08.json
  @echo "✔ range manifest OK"

# Top-level multi-bucket manifest (full mode)
validate-top:
  {{AJV}} validate -s {{SCHEMAS_DIR}}/git-activity-report.full.top.schema.json \
                   -d {{FIXTURES_DIR}}/manifest.json
  @echo "✔ top manifest OK"

# Individual commit shard fixtures (validate both)
validate-commit-shards:
  {{AJV}} validate -s {{SCHEMAS_DIR}}/git-activity-report.commit.schema.json \
                   -d {{FIXTURES_DIR}}/2025.08.12-14.03-aaaaaaaaaaaa.json
  {{AJV}} validate -s {{SCHEMAS_DIR}}/git-activity-report.commit.schema.json \
                   -d {{FIXTURES_DIR}}/2025.08.13-09.12-bbbbbbbbbbbb.json
  @echo "✔ commit shards OK (2/2)"

# All validations in a deterministic order
validate-all: validate-simple validate-commit-shards validate-range validate-top
  @echo "-----------------------------------------------"
  @echo "All validations passed ✓"
  @echo "Schemas:  $(ls -1 {{SCHEMAS_DIR}} | wc -l)   Fixtures: $(ls -1 {{FIXTURES_DIR}} | wc -l)"
  @echo "-----------------------------------------------"

# Optional: Reformat fixtures (idempotent) — requires jq
fmt-fixtures:
  if ! command -v {{JQ}} >/dev/null 2>&1; then \
    echo "jq not found; skipping format."; exit 0; \
  fi
  for f in {{FIXTURES_DIR}}/*.json; do \
    tmp="$${f}.tmp"; \
    {{JQ}} . "$$f" > "$$tmp" && mv "$$tmp" "$$f"; \
    echo "fmt: $$f"; \
  done

# Utility to see what the Justfile resolves
show-env:
  @echo "ROOT        = {{ROOT}}"
  @echo "SCHEMAS_DIR = {{SCHEMAS_DIR}}"
  @echo "FIXTURES_DIR= {{FIXTURES_DIR}}"
  @echo "AJV         = {{AJV}}"
  @echo "JQ          = {{JQ}}"
