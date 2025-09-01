#!/usr/bin/env bash
set -euo pipefail

ROOT=${1:-$(pwd)}
SRC_DIR="$ROOT/src"
missing=0

if [[ ! -d "$SRC_DIR" ]]; then
  echo "No src/ directory found at $ROOT" >&2
  exit 2
fi

while IFS= read -r -d '' f; do
  head=$(head -n 120 "$f")
  # Strip common comment prefixes then search for keys
  norm=$(echo "$head" | sed -E 's/^[[:space:]]*(\/\/|#|\*|\/\*)[[:space:]]*//')
  has_purpose=$(echo "$norm" | grep -Eiq '^purpose:' && echo 1 || echo 0)
  has_role=$(echo "$norm" | grep -Eiq '^role:' && echo 1 || echo 0)
  if [[ $has_purpose -eq 0 || $has_role -eq 0 ]]; then
    echo "[missing headers] $f" >&2
    if [[ $has_purpose -eq 0 ]]; then echo "  - missing: purpose:" >&2; fi
    if [[ $has_role -eq 0 ]]; then echo "  - missing: role:" >&2; fi
    missing=$((missing+1))
  fi
done < <(find "$SRC_DIR" -type f -name '*.rs' -print0)

if [[ $missing -gt 0 ]]; then
  echo "Found $missing file(s) missing required headers." >&2
  exit 1
else
  echo "All module headers present (purpose:, role:)." >&2
fi

