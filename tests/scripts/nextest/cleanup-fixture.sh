#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/../../.. && pwd)"
OUT_DIR="$ROOT_DIR/tests/.tmp"

if [ -f "$OUT_DIR/tmpdir" ]; then
  FIXTURE_DIR="$(cat "$OUT_DIR/tmpdir")"
  rm -f "$OUT_DIR/tmpdir"
  case "$FIXTURE_DIR" in
    /tmp/gar-fixture.*|/private/tmp/gar-fixture.*)
      rm -rf "$FIXTURE_DIR" || true
      ;;
    *)
      echo "[cleanup] refusing to remove non-temporary directory: $FIXTURE_DIR" >&2
      ;;
  esac
fi

