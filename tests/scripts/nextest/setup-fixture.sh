#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/../../.. && pwd)"
OUT_DIR="$ROOT_DIR/tests/.tmp"
mkdir -p "$OUT_DIR"

TMP_DIR="${TMPDIR:-/tmp}"
FIXTURE_DIR="$(mktemp -d "$TMP_DIR/gar-fixture.XXXXXX")"

# Initialize deterministic tiny repo
cd "$FIXTURE_DIR"
git init -q -b main
git config user.name "Fixture Bot"
git config user.email fixture@example.com
git config commit.gpgsign false

mkdir -p app/models
echo "class User; end" > app/models/user.rb
git add .
GIT_AUTHOR_DATE=2025-08-12T14:03:00 GIT_COMMITTER_DATE=2025-08-12T14:03:00 git commit -q -m "feat: add user model"

git checkout -q -b feature/alpha
mkdir -p app/services spec/services
echo "class PaymentService; end" > app/services/payment_service.rb
echo "describe 'PaymentService' do; end" > spec/services/payment_service_spec.rb
git add .
GIT_AUTHOR_DATE=2025-08-13T09:12:00 GIT_COMMITTER_DATE=2025-08-13T09:12:00 git commit -q -m "refactor: extract payment service"

git switch -q -C main

# Persist path for test processes
echo "$FIXTURE_DIR" > "$OUT_DIR/tmpdir"
echo "GAR_FIXTURE_REPO_DIR=$FIXTURE_DIR"

