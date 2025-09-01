// === Module Header (agents-tooling) START ===
// header: Parsed by scripts/check_module_headers.sh for purpose/role presence; keep keys on single-line entries.
// purpose: Provide ergonomic nested JSON fetching via dotted paths and safe typed extraction for serde_json::Value
// role: extension/serde_json
// outputs: JsonFetch trait and JsonFetched wrapper for typed extraction with defaults
// invariants: No panics; missing paths yield None; to_or_default returns T::default on failure
// tie_breakers: contracts > orchestration > correctness > performance > minimal_diffs
// === Module Header END ===

use serde::de::DeserializeOwned;

/// Wrapper around a JSON location to allow typed extraction via a clear second step.
pub struct JsonFetched<'a> {
  inner: Option<&'a serde_json::Value>,
}

impl<'a> JsonFetched<'a> {
  /// Attempt to deserialize the fetched value as `T`.
  pub fn to<T>(&self) -> Option<T>
  where
    T: DeserializeOwned,
  {
    self.inner.and_then(|v| serde_json::from_value::<T>(v.clone()).ok())
  }

  /// Deserialize as `T`, returning `T::default()` on failure.
  pub fn to_or_default<T>(&self) -> T
  where
    T: DeserializeOwned + Default,
  {
    self.to::<T>().unwrap_or_default()
  }
}

/// Extension to fetch nested values via dotted paths like "user.login".
pub trait JsonFetch {
  fn fetch(&self, path: &str) -> JsonFetched<'_>;
}

impl JsonFetch for serde_json::Value {
  fn fetch(&self, path: &str) -> JsonFetched<'_> {
    if path.is_empty() {
      return JsonFetched { inner: Some(self) };
    }

    let mut cur = self;

    for key in path.split('.') {
      match cur.get(key) {
        Some(next) => cur = next,
        None => return JsonFetched { inner: None },
      }
    }

    JsonFetched { inner: Some(cur) }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fetch_top_level_and_nested() {
    let v: serde_json::Value = serde_json::json!({
      "title": "Hello",
      "user": { "login": "octocat" },
      "nums": [1,2,3]
    });

    assert_eq!(v.fetch("title").to::<String>().as_deref(), Some("Hello"));
    assert_eq!(v.fetch("user.login").to::<String>().as_deref(), Some("octocat"));
    assert_eq!(v.fetch("missing").to::<String>(), None);
    assert_eq!(v.fetch("").to::<serde_json::Value>().is_some(), true);
  }

  #[test]
  fn fetch_to_or_default() {
    let v: serde_json::Value = serde_json::json!({});
    let s: String = v.fetch("nope").to_or_default();
    assert_eq!(s, "");
  }
}
