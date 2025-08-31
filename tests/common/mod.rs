use std::path::PathBuf;
use std::process::Command;

#[allow(dead_code)]
pub fn run(repo: &std::path::Path, args: &[&str]) {
  let status = Command::new("git").args(args).current_dir(repo).status().unwrap();
  assert!(status.success(), "git {:?} failed", args);
}

#[allow(dead_code)]
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

// Removed FixtureRepo wrapper: tests now use PathBuf directly.

/// Obtain a fixture repo path for tests.
///
/// Priority:
/// 1) If env GAR_FIXTURE_REPO_DIR is set (e.g., by nextest setup), use it.
/// 2) If tests/.tmp/tmpdir exists, use it.
/// 3) Otherwise create a fresh temp repo (owned for lifetime of this handle).
#[allow(dead_code)]
pub fn fixture_repo() -> PathBuf {
  if let Ok(dir) = std::env::var("GAR_FIXTURE_REPO_DIR") {
    return PathBuf::from(dir);
  }

  let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let tmp_file = manifest_dir.join("tests").join(".tmp").join("tmpdir");
  if let Ok(s) = std::fs::read_to_string(&tmp_file) {
    let p = s.trim();
    if !p.is_empty() {
      return PathBuf::from(p);
    }
  }

  panic!(
    "Fixture repo not found. Ensure nextest setup script has run.\n  - Run: NEXTEST_EXPERIMENTAL_SETUP_SCRIPTS=1 cargo nextest run\n  - Or: bash tests/scripts/nextest/setup-fixture.sh (exports GAR_FIXTURE_REPO_DIR)"
  );
}
