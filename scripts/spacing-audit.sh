#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-normal}"
ROOT="${2:-src}"

echo "[spacing-audit] mode=$MODE root=$ROOT"

run_normal() {
  echo "-- decl→ctrl without blank (phase boundary)"
  rg -nU --pcre2 "^\\s*let\\s+[^;]+;\\n[\\t ]*(if|if\\s+let|for|while|while\\s+let|match)\\b" "$ROOT" | sed -e 's/^/DECL→CTRL: /' || true

  echo "-- compute→I/O after run_git without blank"
  rg -nU --pcre2 "run_git\\([^)]*\\);\\n[\\t ]*[^\\s]" "$ROOT" | sed -e 's/^/IO-BOUNDARY: /' || true

  echo "-- literal build close → push/insert/write without blank"
  rg -nU --pcre2 "}\\s*;\\n[\\t ]*(push|insert|write|println!)\\b" "$ROOT" | sed -e 's/^/BUILD→USE: /' || true
}

run_strict() {
  run_normal

  echo "-- post-block → new control without blank (not else-sibling)"
  find "$ROOT" -name "*.rs" -print0 | while IFS= read -r -d '' f; do
    awk '
      { lines[NR] = $0 }
      END {
        for (i = 1; i <= NR; i++) {
          trim = lines[i]
          gsub(/^\s+|\s+$/, "", trim)
          if (trim == "}") {
            # next non-empty line after optional single blank
            j = i + 1
            if (j <= NR) {
              l1 = lines[j]
              gsub(/^\s+|\s+$/, "", l1)
              had_blank = 0
              if (l1 == "") { had_blank = 1; j = j + 1; l1 = lines[j]; gsub(/^\s+|\s+$/, "", l1) }
              if (l1 ~ /^(if(\s+let)?|for|while(\s+let)?|match)\b/) {
                if (had_blank == 0) {
                  printf("POST-BLOCK→CTRL: %s:%d:  }\n", FILENAME, i)
                  printf("POST-BLOCK→CTRL: %s:%d:\n", FILENAME, i+1)
                  printf("POST-BLOCK→CTRL: %s:%d:  %s\n", FILENAME, j, lines[j])
                }
              }
            }
          }
        }
      }
    ' "$f"
  done

  echo "-- finalization (Ok(...)/return) directly after code without blank"
  find "$ROOT" -name "*.rs" -print0 | while IFS= read -r -d '' f; do
    awk '
      { lines[NR] = $0 }
      END {
        for (i = 2; i <= NR; i++) {
          cur = lines[i]; prev = lines[i-1]
          ctrim = cur; gsub(/^\s+|\s+$/, "", ctrim)
          if (ctrim ~ /^(return\b|Ok\()/) {
            if (prev !~ /^\s*$/) {
              ptrim = prev; gsub(/^\s+|\s+$/, "", ptrim)
              printf("FINALIZE: %s:%d:  %s\n", FILENAME, i-1, ptrim)
              printf("FINALIZE: %s:%d:  %s\n", FILENAME, i, ctrim)
            }
          }
        }
      }
    ' "$f"
  done
}

case "$MODE" in
  normal) run_normal ;;
  strict) run_strict ;;
  *) echo "unknown mode: $MODE" >&2; exit 2 ;;
esac

echo "[spacing-audit] done"
