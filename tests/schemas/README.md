Schemas

This directory contains JSON Schemas used by the test suite to validate the shapes of the tool’s outputs.

- git-activity-report.report.schema.json
  - The unified “report” shape. This is always a simple report containing top‑level metadata (repo, range, include flags, authors, summary) and a commits array. When the CLI is invoked with `--split-apart`, the report also includes an `items` array that indexes per‑commit shard files.

- git-activity-report.commit.schema.json
  - The schema for an individual commit shard file when outputs are split apart. This is the same shape as objects in the `commits[]` array of the report, serialized one‑per‑file.

- git-activity-report.overall.schema.json
  - The “overall manifest” for multi‑range runs (e.g., `--for "every month for the last N months"`). It records the repo, generated_at, include flags, whether outputs were split apart, and a `ranges[]` index. Each range entry includes a label, a start/end range, and a `file` path pointing to the JSON file for that range.

Usage

Tests in `tests/schema_validation.rs` load these schemas and assert that outputs conform. The suite first validates the overall manifest (for multi‑range runs), then validates each per‑range report against the unified report schema, and finally validates commit shard files against the commit schema when present.
