# Use bash with strict flags
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# -------------------------------------------------------------------
# Paths & tooling
# -------------------------------------------------------------------
SCHEMAS_DIR := "tests/schemas"

# Rust binary path
RUST_BIN := "target/debug/git-activity-report"
RUST_BIN_RELEASE := "target/release/git-activity-report"

# -------------------------------------------------------------------
# Help
# -------------------------------------------------------------------
_help:
  @echo "Recipes:"
  @echo "  audit-spacing         # spacing/layout audit (normal)"
  @echo "  audit-spacing-strict  # spacing/layout audit (strict)"
  @echo "  build                 # cargo build"
  @echo "  build-manfile         # generate man page to docs/man/git-activity-report.1"
  @echo "  build-release         # cargo build --release"
  @echo "  check-headers         # verify module headers have purpose/role"
  @echo "  ci-check              # verify cycle presence and headers (CI gate)"
  @echo "  clean                 # clean build objects and data"
  @echo "  clippy                # cargo clippy -D warnings"
  @echo "  clippy-fix            # apply cargo clippy -D warnings"
  @echo "  doctor                # show Rust toolchain info"
  @echo "  fmt                   # cargo fmt --check"
  @echo "  fmt-fix               # cargo fmt"
  @echo "  help                  # print Rust CLI --help (builds first)"
  @echo "  install               # install release binary and man page"
  @echo "  lint-md               # markdown lint"
  @echo "  man-install           # install man page to ~/.local/share/man/man1"
  @echo "  new-cycle             # scaffold a new agent cycle log (Python)"
  @echo "  run-last-week         # example: stdout JSON for last week"
  @echo "  run-month-detailed    # example: detailed month, split, writes to .tmp/out"
  @echo "  style-fix             # run strict spacing audit (non-gating), then cargo fmt"
  @echo "  test                  # run tests (nextest + coverage + schema validation)"
  @echo "  test-all              # run tests without fast-fail (max-fail large)"


doctor:
  set +e
  echo "Rust toolchain:"; rustup show || true

# -------------------------------------------------------------------
# Rust workflow
# -------------------------------------------------------------------

build:
  cargo build --verbose

build-release:
  # Build optimized binary
  cargo build --release

test:
  cargo llvm-cov nextest

clean:
  cargo clean

test-all:
  cargo llvm-cov nextest --max-fail 100000

fmt:
  cargo fmt --all --check

fmt-fix:
  cargo fmt --all

clippy:
  cargo clippy -- -D warnings

clippy-fix:
  cargo clippy --fix -- -D warnings

# Install release binary and man page
install: build-release build-manfile
  # Ensure ~/bin exists
  mkdir -p "$HOME/bin"
  # Install the optimized binary
  install -m 0755 {{RUST_BIN_RELEASE}} "$HOME/bin/git-activity-report"
  echo "Installed binary to $HOME/bin/git-activity-report"
  echo "Ensure $HOME/bin is on your PATH (e.g., add: export PATH=\"$HOME/bin:$PATH\")"

  # Install the man page
  mkdir -p "$HOME/.local/share/man/man1"
  install -m 0644 docs/man/git-activity-report.1 "$HOME/.local/share/man/man1/git-activity-report.1"
  echo "Installed man page to $HOME/.local/share/man/man1/git-activity-report.1"

# Generate man page into docs/man
build-manfile:
  mkdir -p docs/man
  cargo run --quiet -- --gen-man > docs/man/git-activity-report.1
  echo "Wrote docs/man/git-activity-report.1"

# Install man page into user manpath (~/.local/share/man/man1)
man-install: build-manfile
  mkdir -p "$HOME/.local/share/man/man1"
  install -m 0644 docs/man/git-activity-report.1 "$HOME/.local/share/man/man1/git-activity-report.1"
  echo "Installed to $HOME/.local/share/man/man1/git-activity-report.1"
  echo "Tip: update MANPATH if needed, e.g.: export MANPATH=\"$HOME/.local/share/man:$MANPATH\""

# Example: single-window, last week → stdout JSON
run-last-week: build
  {{RUST_BIN}} --for "last week" --repo .

# Example: detailed month with split artifacts
run-month-detailed: build
  {{RUST_BIN}} --month 2025-08 --detailed --split-apart --out .tmp/out --repo .

## Legacy validations and fixture flows removed in favor of Rust-side schema tests.

# Spacing audit (reports potential misses; manual fixes per docs/SPACING.md)
audit-spacing:
  bash scripts/spacing-audit.sh normal src

audit-spacing-strict:
  bash scripts/spacing-audit.sh strict src

audit-spacing-strict-gate:
  bash scripts/spacing-audit.sh strict-gate src

lint-md:
  markdownlint-cli2 --fix "**/*.md"

# Developer convenience: spacing audit (non-gating) followed by rustfmt write.
style-fix:
  bash scripts/spacing-audit.sh strict src || true
  cargo fmt --all

# Scaffold a new agent cycle log from TASK_TEMPLATE
new-cycle desc="work-cycle":
  python scripts/new_cycle.py "{{desc}}"

# Verify required module headers exist
check-headers:
  bash scripts/check_module_headers.sh

ci-check:
  bash scripts/check_cycle_presence.sh
  bash scripts/check_module_headers.sh
