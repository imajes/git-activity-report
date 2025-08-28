#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "usage: $0 <py_dir> <rs_dir> [simple|full]" >&2
  exit 2
fi
PYDIR=$1
RSDIR=$2
MODE=${3:-auto}

ajv() { command ajv --spec=draft2020 "$@"; }

# If mode not supplied, try to infer by presence of manifest.json
if [ "$MODE" = "auto" ]; then
  if [ -f "$PYDIR/manifest.json" ] && [ -f "$RSDIR/manifest.json" ]; then
    MODE=full
  else
    MODE=simple
  fi
fi

if [ "$MODE" = "simple" ]; then
  # Find first JSON file in each dir
  PYFILE=$(ls "$PYDIR"/*.json | head -n1)
  RSFILE=$(ls "$RSDIR"/*.json | head -n1)
  if [ -z "${PYFILE:-}" ] || [ -z "${RSFILE:-}" ]; then
    echo "could not locate simple json files in $PYDIR or $RSDIR" >&2
    exit 1
  fi
  # Validate
  ajv validate -s tests/schemas/git-activity-report.simple.schema.json -d "$PYFILE"
  ajv validate -s tests/schemas/git-activity-report.simple.schema.json -d "$RSFILE"
  # Diff sorted
  diff -u <(jq -S . "$PYFILE") <(jq -S . "$RSFILE") || true
  exit 0
fi

if [ "$MODE" = "full" ]; then
  # Validate top manifests
  ajv validate -s tests/schemas/git-activity-report.full.top.schema.json -d "$PYDIR/manifest.json"
  ajv validate -s tests/schemas/git-activity-report.full.top.schema.json -d "$RSDIR/manifest.json"
  # For each range manifest, validate and compare items(subjects by sha)
  py_items=$(jq -c '[.buckets[].manifest] | unique' "$PYDIR/manifest.json")
  rs_items=$(jq -c '[.buckets[].manifest] | unique' "$RSDIR/manifest.json")
  for mf in $(jq -r '.[]' <<< "$py_items"); do
    ajv validate -s tests/schemas/git-activity-report.full.range.schema.json -d "$PYDIR/$mf"
  done
  for mf in $(jq -r '.[]' <<< "$rs_items"); do
    ajv validate -s tests/schemas/git-activity-report.full.range.schema.json -d "$RSDIR/$mf"
  done
  # Compare items lists by sha+subject
  norm_items() { jq -S '[.items[] | {sha, subject}] | sort_by(.sha)' "$1"; }
  for mf in $(jq -r '.[]' <<< "$py_items"); do
    if [ -f "$RSDIR/$mf" ]; then
      echo "-- compare $mf"
      diff -u <(norm_items "$PYDIR/$mf") <(norm_items "$RSDIR/$mf") || true
    else
      echo "(warn) missing in RS: $mf"
    fi
  done
  exit 0
fi

echo "unknown mode: $MODE" >&2
exit 2

