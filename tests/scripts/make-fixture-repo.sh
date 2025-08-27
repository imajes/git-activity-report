#!/usr/bin/env bash
set -euo pipefail

# Deterministic tiny repo + fixture emission using the Python reference
# Requires: git, python3, jq (for formatting in downstream tasks)

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)

echo "[fixtures] creating repo at: $TMP"
cd "$TMP"
git init -q
git config user.name "Fixture Bot"
git config user.email fixture@example.com

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

# Back to main for HEAD view
git checkout -q -b main

cd "$ROOT"
mkdir -p tests/fixtures .tmp
echo "$TMP" > .tmp/tmpdir

# Simple fixture (Python reference)
python3 ./git-activity-report.py \
  --simple \
  --since "2025-08-01" --until "2025-09-01" \
  --repo "$TMP" > tests/fixtures/git-activity-report.simple.fixture.json

# Full fixtures into a staging dir, then copy interesting files
OUTDIR=tests/fixtures/full_out
mkdir -p "$OUTDIR"
python3 ./git-activity-report.py \
  --full \
  --since "2025-08-01" --until "2025-09-01" \
  --repo "$TMP" --split-out "$OUTDIR" --include-unmerged > /dev/null

cp "$OUTDIR/manifest.json" tests/fixtures/ || true
cp "$OUTDIR/manifest-2025-08.json" tests/fixtures/ || true

# Copy one HEAD shard and one unmerged shard to stable filenames for examples
HEAD_SHARD=$(ls "$OUTDIR/2025-08/2025.08.12-14.03-"*.json 2>/dev/null | head -n 1 || true)
UNMERGED_SHARD=$(ls "$OUTDIR/2025-08/unmerged/"*/2025.08.13-09.12-*.json 2>/dev/null | head -n 1 || true)

if [[ -f "$HEAD_SHARD" ]]; then
  cp "$HEAD_SHARD" tests/fixtures/2025.08.12-14.03-aaaaaaaaaaaa.json
fi
if [[ -f "$UNMERGED_SHARD" ]]; then
  cp "$UNMERGED_SHARD" tests/fixtures/2025.08.13-09.12-bbbbbbbbbbbb.json
fi

echo "Fixture repo at: $TMP (A=$A_SHA B=$B_SHA)"
