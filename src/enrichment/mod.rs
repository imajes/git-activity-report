// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Namespace for enrichment features (GitHub PRs, etc.)
// role: enrichment/namespace
// outputs: Public submodules implementing specific enrichments
// invariants: Each enrichment isolates external integrations and remains best-effort
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

pub mod github_pull_requests;
pub mod github_api;
pub mod effort;
