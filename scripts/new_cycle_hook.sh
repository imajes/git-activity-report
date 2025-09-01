#!/usr/bin/env bash
set -euo pipefail

# Usage:
#  scripts/new_cycle_hook.sh --desc "short description" [--ask-file path] [--append]
#  scripts/new_cycle_hook.sh --stdin  (reads ask text from stdin and derives description)
#
# Behavior:
#  - Ensures a new cycle file exists by calling `just new-cycle` (or scripts/new_cycle.sh)
#  - Exports AGENT_CYCLE_FILE and AGENT_OVERLAY_JSON for the calling process
#  - Optionally appends the raw ask into the new cycle file under an "Ask" section

desc=""
ask_file=""
from_stdin=0
append=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --desc) shift; desc=${1:-};;
    --ask-file) shift; ask_file=${1:-};;
    --stdin) from_stdin=1;;
    --append) append=1;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
  shift || true
done

if [[ $from_stdin -eq 1 ]]; then
  tmp=$(mktemp)
  cat > "$tmp"
  ask_file="$tmp"
fi

if [[ -z "$desc" ]]; then
  if [[ -n "$ask_file" && -f "$ask_file" ]]; then
    # derive a short description from the first line (up to ~8 words)
    desc=$(head -n1 "$ask_file" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9 ]+/ /g; s/^ +//; s/ +$//; s/ +/ /g' | awk '{ for (i=1; i<=NF && i<=8; i++) printf (i==1?"%s":"-%s"), $i }')
    desc=${desc:-"work-cycle"}
  else
    desc="work-cycle"
  fi
fi

# repo root
if git rev-parse --show-toplevel >/dev/null 2>&1; then
  GITROOT=$(git rev-parse --show-toplevel)
else
  GITROOT=$(pwd)
fi

if command -v just >/dev/null 2>&1; then
  just new-cycle "$desc" >/dev/null
else
  bash "$(dirname "$0")/new_cycle.sh" "$desc" >/dev/null
fi

CYC_DIR="$GITROOT/.agents/cycles"
latest=$(ls -1 "$CYC_DIR"/*.md | sort | tail -n1)
overlay="$GITROOT/.agents/repo_overlay.json"

if [[ $append -eq 1 && -n "$ask_file" && -f "$ask_file" ]]; then
  {
    echo
    echo "### Ask"
    echo
    cat "$ask_file"
    echo
  } >> "$latest"
fi

export AGENT_CYCLE_FILE="$latest"
export AGENT_OVERLAY_JSON="$overlay"

echo "$latest"

