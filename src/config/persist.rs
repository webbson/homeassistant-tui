use std::path::Path;

use color_eyre::eyre::Context;
use color_eyre::Result;

use super::Config;

pub fn save(cfg: &Config, path: &Path) -> Result<()> {
    let yaml = serde_yaml::to_string(cfg).context("serialize config")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    std::fs::write(path, yaml).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
