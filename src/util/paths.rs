use std::path::PathBuf;

/// XDG-style config dir: `$XDG_CONFIG_HOME/ha-tui` or `~/.config/ha-tui`.
/// Identical on Linux and macOS — we do NOT use the Apple-style
/// `~/Library/Application Support` path for user-editable config.
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("ha-tui"))
}

/// Data dir for logs/state: `$XDG_DATA_HOME/ha-tui` or
/// `~/Library/Application Support/ha-tui` on macOS / `~/.local/share/ha-tui` on Linux.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(d).join("ha-tui"));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    #[cfg(target_os = "macos")]
    {
        Some(home.join("Library/Application Support/ha-tui"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(home.join(".local/share/ha-tui"))
    }
}
