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
    self
      .inner
      .and_then(|v| serde_json::from_value::<T>(v.clone()).ok())
  }

  /// Deserialize as `T`, returning `T::default()` on failure.
  pub fn to_or_default<T>(&self) -> T
  where
    T: DeserializeOwned + Default,
  {
    self
      .to::<T>()
      .unwrap_or_default()
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

