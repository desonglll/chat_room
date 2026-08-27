use anyhow::{bail, Result};
use serde::Deserialize;

const DEPENDENCIES: [&str; 3] = ["redis", "vector_store", "ai_provider"];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub json_logs: bool,
    pub required_dependencies: Vec<String>,
}

impl ObservabilityConfig {
    pub(crate) fn is_required(&self, dependency: &str) -> bool {
        self.required_dependencies
            .iter()
            .any(|configured| configured == dependency)
    }

    pub(super) fn validate(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for dependency in &self.required_dependencies {
            if !DEPENDENCIES.contains(&dependency.as_str()) {
                bail!(
                    "observability.required_dependencies supports only redis, vector_store, and ai_provider"
                );
            }
            if !seen.insert(dependency) {
                bail!("observability.required_dependencies must not contain duplicates");
            }
        }
        Ok(())
    }
}
