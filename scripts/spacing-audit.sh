#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-normal}"
ROOT="${2:-src}"

echo "[spacing-audit] mode=$MODE root=$ROOT"

run_normal() {
  echo "-- decl→ctrl without blank (phase boundary)"
  rg -nU "^\\s*let\\s+[^;]+;\\n[\\t ]*(if|if\\s+let|for|while|while\\s+let|match)\\b" "$ROOT" | sed -e 's/^/DECL→CTRL: /' || true

  echo "-- compute→I/O after run_git without blank"
  rg -nU "run_git\\([^)]*\\);\\n[\\t ]*[^\\s]" "$ROOT" | sed -e 's/^/IO-BOUNDARY: /' || true

  echo "-- literal build close → push/insert/write without blank"
  rg -nU "}\\s*;\\n[\\t ]*(push|insert|write|println!)\\b" "$ROOT" | sed -e 's/^/BUILD→USE: /' || true
}

run_strict() {
  run_normal

  echo "-- post-block → new control without blank (not else-sibling)"
  rg -nU "^\\s*}\\s*\\n[\\t ]*(if|if\\s+let|for|while|while\\s+let|match)\\b" "$ROOT" | sed -e 's/^/POST-BLOCK→CTRL: /' || true

  echo "-- finalization (Ok(...)/return) directly after code without blank"
  rg -nU "^[^\\n{}].+\\n[\\t ]*(Ok\\(|return\\b)" "$ROOT" | sed -e 's/^/FINALIZE: /' || true
}

case "$MODE" in
  normal) run_normal ;;
  strict) run_strict ;;
  *) echo "unknown mode: $MODE" >&2; exit 2 ;;
esac

echo "[spacing-audit] done"

