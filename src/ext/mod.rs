// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Group extension traits and helpers for third-party crates and std types under a single `ext` namespace
// role: module/aggregation
// outputs: Re-exported submodules providing utility traits and helpers (e.g., JsonFetch)
// invariants: No side effects; pure extensions only
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

// Extension modules for third-party crates and std types.
// Group all extension traits and helpers under `crate::ext`.

pub mod serde_json;
