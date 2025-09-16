#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-normal}"
ROOT="${2:-src}"
HAD_FINDINGS=0

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

check_gate_normal() {
  rg -q -nU --pcre2 "^\\s*let\\s+[^;]+;\\n[\\t ]*(if|if\\s+let|for|while|while\\s+let|match)\\b" "$ROOT" && HAD_FINDINGS=1 || true
  rg -q -nU --pcre2 "run_git\\([^)]*\\);\\n[\\t ]*[^\\s]" "$ROOT" && HAD_FINDINGS=1 || true
  rg -q -nU --pcre2 "}\\s*;\\n[\\t ]*(push|insert|write|println!)\\b" "$ROOT" && HAD_FINDINGS=1 || true
}

check_gate_strict() {
  check_gate_normal

  tmpfile=$(mktemp)
  # post-block → new control
  find "$ROOT" -name "*.rs" -print0 | while IFS= read -r -d '' f; do
    awk '
      { lines[NR] = $0 }
      END {
        for (i = 1; i <= NR; i++) {
          trim = lines[i]
          gsub(/^\s+|\s+$/, "", trim)
          if (trim == "}") {
            j = i + 1
            if (j <= NR) {
              l1 = lines[j]
              gsub(/^\s+|\s+$/, "", l1)
              had_blank = 0
              if (l1 == "") { had_blank = 1; j = j + 1; l1 = lines[j]; gsub(/^\s+|\s+$/, "", l1) }
              if (l1 ~ /^(if(\s+let)?|for|while(\s+let)?|match)\b/) {
                if (had_blank == 0) {
                  printf("POST-BLOCK→CTRL: %s:%d\n", FILENAME, i)
                }
              }
            }
          }
        }
      }
    ' "$f" >>"$tmpfile"
  done
  if [[ -s "$tmpfile" ]]; then HAD_FINDINGS=1; fi
  rm -f "$tmpfile"

  tmpfile2=$(mktemp)
  # finalization directly after code without blank
  find "$ROOT" -name "*.rs" -print0 | while IFS= read -r -d '' f; do
    awk '
      { lines[NR] = $0 }
      END {
        for (i = 2; i <= NR; i++) {
          cur = lines[i]; prev = lines[i-1]
          ctrim = cur; gsub(/^\s+|\s+$/, "", ctrim)
          if (ctrim ~ /^(return\b|Ok\()/) {
            if (prev !~ /^\s*$/) {
              printf("FINALIZE: %s:%d\n", FILENAME, i)
            }
          }
        }
      }
    ' "$f" >>"$tmpfile2"
  done
  if [[ -s "$tmpfile2" ]]; then HAD_FINDINGS=1; fi
  rm -f "$tmpfile2"
}

case "$MODE" in
  normal)
    run_normal ;;
  strict)
    run_strict ;;
  normal-gate)
    run_normal
    check_gate_normal ;;
  strict-gate)
    run_strict
    check_gate_strict ;;
  *) echo "unknown mode: $MODE" >&2; exit 2 ;;
esac

if [[ "$MODE" == *"gate"* ]]; then
  if [[ $HAD_FINDINGS -ne 0 ]]; then
    echo "[spacing-audit] findings detected — gate failing" >&2
    exit 1
  fi
fi

echo "[spacing-audit] done"
