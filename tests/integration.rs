// Driver for integration + snapshot tests under tests/integration/
// Keeps tests organized in a subdirectory while remaining visible to Cargo.
//
#[path = "integration/cli_gen_man.rs"]
mod cli_gen_man;
#[path = "integration/cli_windows.rs"]
mod cli_windows;
#[path = "integration/for_phrases.rs"]
mod for_phrases;
#[path = "integration/full_unmerged.rs"]
mod full_unmerged;
#[path = "integration/overall_manifest.rs"]
mod overall_manifest;
#[path = "integration/patch_behaviors.rs"]
mod patch_behaviors;
#[path = "integration/report_end_to_end.rs"]
mod report_end_to_end;
#[path = "integration/schema_validation.rs"]
mod schema_validation;

// snapshots
#[path = "integration/cli_full_snapshot.rs"]
mod cli_full_snapshot;
#[path = "integration/cli_simple_snapshot.rs"]
mod cli_simple_snapshot;
#[path = "integration/full_manifest_snapshot.rs"]
mod full_manifest_snapshot;
#[path = "integration/full_shard_snapshot.rs"]
mod full_shard_snapshot;
#[path = "integration/simple_snapshot.rs"]
mod simple_snapshot;
