/// The set of `native.*` API paths that are permitted to execute.
/// Built from the `--allow` CLI flag or programmatically.
#[derive(Debug, Default, Clone)]
pub struct PermissionManifest {
    allowed: std::collections::HashSet<String>,
}

impl PermissionManifest {
    /// Build from a list of strings like `["native.log", "native.fs.read"]`.
    pub fn new(allow: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowed: allow.into_iter().collect(),
        }
    }

    /// An open manifest that permits everything — useful for testing.
    pub fn allow_all() -> Self {
        Self { allowed: std::collections::HashSet::new() }
    }

    /// Returns true if `api` (e.g. `"native.fs.read"`) is permitted.
    /// If the manifest is empty, everything is allowed (opt-in restriction model).
    pub fn is_allowed(&self, api: &str) -> bool {
        self.allowed.is_empty() || self.allowed.contains(api)
    }
}
