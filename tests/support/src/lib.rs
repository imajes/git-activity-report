//! test-support: helpers for robust, nextest-friendly tests.
//!
//! Add as a dev-dependency in your top-level `Cargo.toml`:
//!
//! ```toml
//! [dev-dependencies]
//! test-support = { path = "tests/support", features = ["serde", "tokio"] }
//! ```
//!
//! Then in tests:
//! ```rust
//! use test_support::{init_tracing, fixtures_dir, read_fixture_json};
//!
//! #[test]
//! fn example() {
//!     init_tracing();
//!     let _root = fixtures_dir();
//! }
//! ```

use once_cell::sync::Lazy;
use camino::Utf8PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

use std::{env, path::{Path, PathBuf}};
use std::process::Command;

/// Initialize `tracing` once, honoring `RUST_LOG` and writing via the test writer.
///
/// Safe to call from multiple tests; only the first call configures the global subscriber.
pub fn init_tracing() {
    static INIT: Lazy<()> = Lazy::new(|| {
        let filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("warn,test=info"))
            .unwrap();
        // with_test_writer() causes logs to appear alongside failing tests only (cargo/nextest)
        let _ = fmt().with_env_filter(filter).with_test_writer().try_init();
    });
    Lazy::force(&INIT);
}

/// Initialize insta snapshot settings once per test process.
///
/// - Centralizes snapshot files in `tests/snapshots` (relative to the test binary's CWD)
/// - Omits `Expression:` in snapshot headers for cleaner diffs
pub fn init_insta() {
    static INIT: Lazy<()> = Lazy::new(|| {
        let mut settings = insta::Settings::clone_current();
        // Point to the central snapshots directory in the workspace
        settings.set_snapshot_path("../snapshots");
        settings.set_omit_expression(true);
        // Bind settings to the thread for the remainder of the test process by leaking the guard
        let guard = settings.bind_to_scope();
        std::mem::forget(guard);
    });
    Lazy::force(&INIT);
}

/// Return the path to the repository's `tests/fixtures` directory.
///
/// Uses the package directory (where `Cargo.toml` lives), so it's stable regardless
/// of the runner's working directory (cargo vs nextest).
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

/// Read a UTF-8 text fixture into a string.
pub fn read_fixture_text<P: AsRef<Path>>(rel_path: P) -> String {
    let path = fixtures_dir().join(rel_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

/// Read a binary fixture into bytes.
pub fn read_fixture_bytes<P: AsRef<Path>>(rel_path: P) -> Vec<u8> {
    let path = fixtures_dir().join(rel_path);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

/// Deserialize a JSON fixture into `T` (enable `serde` feature).
#[cfg(feature = "serde")]
pub fn read_fixture_json<T, P>(rel_path: P) -> T
where
    T: serde::de::DeserializeOwned,
    P: AsRef<Path>,
{
    let path = fixtures_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("failed to open fixture {}: {e}", path.display()));
    serde_json::from_reader::<_, T>(file)
        .unwrap_or_else(|e| panic!("failed to parse JSON fixture {}: {e}", path.display()))
}

/// Create a temp directory that deletes on drop.
pub fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create tempdir")
}

/// Create (and return) a temp working directory for CLI tests.
/// Also sets CWD to that directory for the duration of `_guard`'s lifetime.
pub fn temp_cwd() -> (tempfile::TempDir, CwdGuard) {
    let td = tempdir();
    let guard = CwdGuard::push(td.path());
    (td, guard)
}

/// Set multiple environment variables for the duration of the returned guard.
pub fn with_env(vars: &[(&str, &str)]) -> EnvGuard {
    EnvGuard::set_many(vars)
}

/// Run a binary target with `assert_cmd`, returning the ready-to-run `Command`.
///
/// Example:
/// ```
/// use test_support::cmd_bin;
/// use predicates::prelude::*;
///
/// let mut cmd = cmd_bin("my-cli");
/// cmd.arg("--help").assert().success().stdout(predicate::str::contains("USAGE"));
/// ```
pub fn cmd_bin(bin: &str) -> assert_cmd::Command {
    init_tracing();
    assert_cmd::Command::cargo_bin(bin).expect("binary target not found")
}

/// Resolve a path inside a temp directory in a platform-safe way (UTF-8).
pub fn utf8_join(base: &Path, rel: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(base.join(rel)).expect("valid UTF-8 path")
}

/// Guard that restores the previous current working directory when dropped.
pub struct CwdGuard {
    prev: PathBuf,
}

impl CwdGuard {
    pub fn push<P: AsRef<Path>>(new_dir: P) -> Self {
        let prev = env::current_dir().expect("cwd");
        env::set_current_dir(&new_dir).unwrap_or_else(|e| {
            panic!("failed to set cwd to {}: {e}", new_dir.as_ref().display())
        });
        Self { prev }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.prev);
    }
}

/// Guard for temporarily setting environment variables.
pub struct EnvGuard {
    prev: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    pub fn set_many(kv: &[(&str, &str)]) -> Self {
        let mut prev = Vec::with_capacity(kv.len());
        for (k, v) in kv {
            let k_owned = k.to_string();
            prev.push((k_owned.clone(), env::var(k).ok()));
            env::set_var(k, v);
        }
        Self { prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, old) in self.prev.drain(..) {
            match old {
                Some(v) => env::set_var(&k, v),
                None => env::remove_var(&k),
            }
        }
    }
}

// --- Optional Tokio helpers (feature = "tokio") ---

/// Pause tokio time for deterministic time-based async tests.
#[cfg(feature = "tokio")]
pub fn tokio_time_pause() {
    tokio::time::pause();
}

/// Advance tokio time by `dur`.
#[cfg(feature = "tokio")]
pub async fn tokio_time_advance(dur: std::time::Duration) {
    tokio::time::advance(dur).await;
}

/// imported from tests/common/mod.rs
pub fn run(repo: &std::path::Path, args: &[&str]) {
  let status = Command::new("git").args(args).current_dir(repo).status().unwrap();
  assert!(status.success(), "git {:?} failed", args);
}

pub fn init_fixture_repo() -> tempfile::TempDir {
  let dir = tempfile::TempDir::new().unwrap();

  // init repo
  run(dir.path(), &["init", "-q", "-b", "main"]);
  run(dir.path(), &["config", "user.name", "Fixture Bot"]);
  run(dir.path(), &["config", "user.email", "fixture@example.com"]);
  run(dir.path(), &["config", "commit.gpgsign", "false"]);

  // Commit A
  std::fs::create_dir_all(dir.path().join("app/models")).unwrap();
  std::fs::write(dir.path().join("app/models/user.rb"), "class User; end\n").unwrap();

  run(dir.path(), &["add", "."]);

  let env = [
    ("GIT_AUTHOR_DATE", "2025-08-12T14:03:00"),
    ("GIT_COMMITTER_DATE", "2025-08-12T14:03:00"),
  ];

  let status = Command::new("git")
    .arg("commit")
    .arg("-q")
    .arg("-m")
    .arg("feat: add user model")
    .current_dir(dir.path())
    .envs(env.iter().cloned())
    .status()
    .unwrap();

  assert!(status.success());

  // Branch and commit B
  run(dir.path(), &["checkout", "-q", "-b", "feature/alpha"]);

  std::fs::create_dir_all(dir.path().join("app/services")).unwrap();
  std::fs::create_dir_all(dir.path().join("spec/services")).unwrap();

  std::fs::write(
    dir.path().join("app/services/payment_service.rb"),
    "class PaymentService; end\n",
  )
  .unwrap();

  std::fs::write(
    dir.path().join("spec/services/payment_service_spec.rb"),
    "describe 'PaymentService' do; end\n",
  )
  .unwrap();

  run(dir.path(), &["add", "."]);

  let env2 = [
    ("GIT_AUTHOR_DATE", "2025-08-13T09:12:00"),
    ("GIT_COMMITTER_DATE", "2025-08-13T09:12:00"),
  ];

  let status = Command::new("git")
    .arg("commit")
    .arg("-q")
    .arg("-m")
    .arg("refactor: extract payment service")
    .current_dir(dir.path())
    .envs(env2.iter().cloned())
    .status()
    .unwrap();

  assert!(status.success());

  // Back to main

  run(dir.path(), &["switch", "-q", "-C", "main"]);

  dir
}

/// Obtain a fixture repo path for tests.
///
/// Priority:
/// 1) If env GAR_FIXTURE_REPO_DIR is set (e.g., by nextest setup), use it.
/// 2) If tests/.tmp/tmpdir exists, use it.
/// 3) Otherwise create a fresh temp repo (owned for lifetime of this handle).
pub fn fixture_repo() -> PathBuf {
  if let Ok(dir) = std::env::var("GAR_FIXTURE_REPO_DIR") {
    return PathBuf::from(dir);
  }

  // Try top-level tests/.tmp/tmpdir relative to the workspace root.
  let support_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  // Common layout: <repo>/tests/support (manifest dir) → parent() is <repo>/tests
  if let Some(top_tests_dir) = support_manifest_dir.parent() {
    let tmp_file = top_tests_dir.join(".tmp").join("tmpdir");
    if let Ok(s) = std::fs::read_to_string(&tmp_file) {
      let p = s.trim();
      if !p.is_empty() {
        return PathBuf::from(p);
      }
    }
  }

  // Fallback to legacy path (in case layout differs): tests/support/tests/.tmp/tmpdir
  let legacy_tmp_file = support_manifest_dir.join("tests").join(".tmp").join("tmpdir");
  if let Ok(s) = std::fs::read_to_string(&legacy_tmp_file) {
    let p = s.trim();
    if !p.is_empty() {
      return PathBuf::from(p);
    }
  }

  panic!(
    "Fixture repo not found. Ensure nextest setup script has run.\n  - Run: cargo nextest run\n  - Or: bash tests/scripts/nextest/setup-fixture.sh (exports GAR_FIXTURE_REPO_DIR)"
  );
}
