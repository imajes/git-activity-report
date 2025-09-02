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
  @echo "  install       # build release and copy binary to ~/bin"
  @echo "  man           # generate man page to docs/man/git-activity-report.1"
  @echo "  man-install   # install man page to ~/.local/share/man/man1"
  @echo "  test          # cargo test"
  @echo "  fmt           # cargo fmt --check"
  @echo "  fmt-fix       # cargo fmt"
  @echo "  clippy        # cargo clippy -D warnings"
  @echo "  help          # print Rust CLI --help (builds first)"
  @echo "  run-simple    # sample run of Rust CLI (prints normalized config)"
  @echo "  run-full      # sample full-mode run (config print for now)"
  @echo "  test          # run tests (nextest + coverage + schema validation)"
  @echo "  new-cycle     # scaffold a new agent cycle log"
  @echo "  check-headers # verify module headers have purpose/role"
  @echo "  ci-check      # verify cycle presence and headers (CI gate)"

doctor:
  set +e
  echo "Rust toolchain:"; rustup show || true

# -------------------------------------------------------------------
# Rust workflow
# -------------------------------------------------------------------

build:
  cargo build

test:
  cargo llvm-cov nextest

test-all:
  cargo llvm-cov nextest --max-fail 100000

fmt:
  cargo fmt --all --check

fmt-fix:
  cargo fmt --all

clippy:
  cargo clippy -- -D warnings

# Install release binary into ~/bin
install:
  # Build optimized binary
  cargo build --release
  # Ensure ~/bin exists
  mkdir -p "$HOME/bin"
  # Copy the binary with execute permissions
  install -m 0755 target/release/git-activity-report "$HOME/bin/git-activity-report"
  echo "Installed to $HOME/bin/git-activity-report"
  echo "Ensure $HOME/bin is on your PATH (e.g., add: export PATH=\"$HOME/bin:$PATH\")"

# Generate man page into docs/man
man:
  mkdir -p docs/man
  cargo run --quiet -- --gen-man > docs/man/git-activity-report.1
  echo "Wrote docs/man/git-activity-report.1"

# Install man page into user manpath (~/.local/share/man/man1)
man-install: man
  mkdir -p "$HOME/.local/share/man/man1"
  install -m 0644 docs/man/git-activity-report.1 "$HOME/.local/share/man/man1/git-activity-report.1"
  echo "Installed to $HOME/.local/share/man/man1/git-activity-report.1"
  echo "Tip: update MANPATH if needed, e.g.: export MANPATH=\"$HOME/.local/share/man:$MANPATH\""

# Sample: print normalized config for a simple window
run-simple: build
  {{RUST_BIN}} --simple --for "last week" --repo .

# Sample: print normalized config for a full window
run-full: build
  {{RUST_BIN}} --full --month 2025-08 --out .tmp/out

## Legacy validations and fixture flows removed in favor of Rust-side schema tests.

# Spacing audit (reports potential misses; manual fixes per docs/SPACING.md)
audit-spacing:
  bash scripts/spacing-audit.sh normal src

audit-spacing-strict:
  bash scripts/spacing-audit.sh strict src

lint-md:
  markdownlint-cli2 --fix "**/*.md"

# Scaffold a new agent cycle log from TASK_TEMPLATE
new-cycle desc="work-cycle":
  bash scripts/new_cycle.sh "{{desc}}"

# Verify required module headers exist
check-headers:
  bash scripts/check_module_headers.sh

ci-check:
  bash scripts/check_cycle_presence.sh
  bash scripts/check_module_headers.sh
