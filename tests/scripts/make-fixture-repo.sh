#!/usr/bin/env bash
set -euo pipefail

# Deterministic tiny repo + fixture emission using the Python reference
# Requires: git, python3, jq (for formatting in downstream tasks)

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)

echo "[fixtures] creating repo at: $TMP"
cd "$TMP"
git init -q -b main
git config user.name "Fixture Bot"
git config user.email fixture@example.com
git config commit.gpgsign false

# Commit A (on default branch)
mkdir -p app/models
printf "class User; end\n" > app/models/user.rb
git add .
GIT_AUTHOR_DATE="2025-08-12T14:03:00" GIT_COMMITTER_DATE="2025-08-12T14:03:00" \
  git commit -q -m "feat: add user model"
A_SHA=$(git rev-parse HEAD)

# Commit B on feature branch (unmerged)
git checkout -q -b feature/alpha
mkdir -p app/services spec/services
printf "class PaymentService; end\n" > app/services/payment_service.rb
printf "describe 'PaymentService' do; end\n" > spec/services/payment_service_spec.rb
git add .
GIT_AUTHOR_DATE="2025-08-13T09:12:00" GIT_COMMITTER_DATE="2025-08-13T09:12:00" \
  git commit -q -m "refactor: extract payment service"
B_SHA=$(git rev-parse HEAD)

# Back to main for HEAD view (ensure present)
git switch -q -C main

cd "$ROOT"
mkdir -p tests/fixtures .tmp tests/.tmp
echo "$TMP" > .tmp/tmpdir
echo "$TMP" > tests/.tmp/tmpdir

# Simple JSON into a temp staging area (do not overwrite committed fixtures)
OUTTMP=tests/.tmp/out
mkdir -p "$OUTTMP"
python3 ./git-activity-report.py \
  --simple \
  --since "2025-08-01" --until "2025-09-01" \
  --repo "$TMP" > "$OUTTMP/git-activity-report.simple.json"

# Full output into a temp staging dir
OUTDIR=tests/.tmp/full_out
mkdir -p "$OUTDIR"
python3 ./git-activity-report.py \
  --full \
  --since "2025-08-01" --until "2025-09-01" \
  --repo "$TMP" --split-out "$OUTDIR" --include-unmerged > /dev/null

echo "Fixture repo at: $TMP (A=$A_SHA B=$B_SHA)"
