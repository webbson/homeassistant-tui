pub mod load;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type Alias = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub instances: Vec<InstanceConfig>,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub dashboards_path: Option<PathBuf>,
}

fn default_log_level() -> String {
    "info".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub alias: Alias,
    pub url: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub token_file: Option<PathBuf>,
    #[serde(default)]
    pub color: Option<String>,
}

impl Config {
    pub fn instance_colors(&self) -> IndexMap<Alias, Option<String>> {
        self.instances
            .iter()
            .map(|i| (i.alias.clone(), i.color.clone()))
            .collect()
    }
}
