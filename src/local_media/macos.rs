use std::path::PathBuf;
use std::time::Duration;

use tokio::process::Command;

use super::{LocalCommand, LocalMediaSnapshot};

const APPS: [&str; 2] = ["Music", "Spotify"];

pub async fn fetch_snapshot() -> LocalMediaSnapshot {
    for app in APPS {
        if let Some(snap) = try_fetch(app).await {
            return snap;
        }
    }
    LocalMediaSnapshot::default()
}

async fn try_fetch(app: &str) -> Option<LocalMediaSnapshot> {
    // First check if the app is running and playing/paused
    let state_script =
        format!("tell application \"{app}\" to if it is running then get player state as string");
    let state_out = run_osascript(&state_script).await?;
    let state = state_out.trim().to_lowercase();
    if state != "playing" && state != "paused" {
        return None;
    }
    let is_playing = state == "playing";

    // Fetch track metadata in one call.
    // Both Spotify and Music.app report duration in seconds via AppleScript.
    // Wrap player position and sound volume in try blocks — some Spotify versions
    // don't support sound volume and would cause the whole script to fail.
    let meta_script = format!(
        r#"tell application "{app}"
set t to current track
set n to name of t
set ar to artist of t
set al to album of t
set pos to 0
try
    set pos to player position
end try
set dur to (duration of t) as string
set vol to 0
try
    set vol to sound volume
end try
return n & "|||" & ar & "|||" & al & "|||" & (pos as string) & "|||" & dur & "|||" & (vol as string)
end tell"#
    );
    let meta_out = run_osascript(&meta_script).await?;
    let parts: Vec<&str> = meta_out.trim().splitn(6, "|||").collect();
    if parts.len() < 6 {
        return None;
    }

    let title = non_empty(parts[0]);
    let artist = non_empty(parts[1]);
    let album = non_empty(parts[2]);
    // Spotify AppleScript reports duration in milliseconds; player position is in seconds.
    let position_secs = parse_f64(parts[3]);
    let duration_secs = parse_f64(parts[4]).map(|v| if app == "Spotify" { v / 1000.0 } else { v });
    let volume_raw = parse_f64(parts[5]);
    let volume_0_1 = volume_raw.map(|v| v / 100.0);

    let art_path = if app == "Music" {
        fetch_music_artwork().await
    } else if app == "Spotify" {
        fetch_spotify_artwork().await
    } else {
        None
    };

    Some(LocalMediaSnapshot {
        player_name: app.to_string(),
        title,
        artist,
        album,
        position_secs,
        duration_secs,
        volume_0_1,
        is_muted: false,
        is_playing,
        art_path,
    })
}

async fn fetch_music_artwork() -> Option<PathBuf> {
    let path = std::env::temp_dir().join("ha-tui-art.jpg");
    let path_str = path.to_string_lossy();
    let script = format!(
        r#"tell application "Music"
set f to open for access POSIX file "{path_str}" with write permission
set eof f to 0
write (raw data of artwork 1 of current track) to f
close access f
end tell"#
    );
    run_osascript(&script).await?;
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

async fn fetch_spotify_artwork() -> Option<PathBuf> {
    // Spotify exposes a CDN URL via AppleScript; download it once per URL change.
    let url_script = r#"tell application "Spotify" to get artwork url of current track"#;
    let url_out = run_osascript(url_script).await?;
    let url = url_out.trim().to_string();
    if url.is_empty() || url == "missing value" {
        return None;
    }

    let path = std::env::temp_dir().join("ha-tui-spotify-art.jpg");

    // Re-use cached file if the URL token embedded in the filename suffix matches.
    // Store the last-downloaded URL alongside the image so we don't re-download.
    let url_cache = std::env::temp_dir().join("ha-tui-spotify-art.url");
    if let Ok(cached_url) = std::fs::read_to_string(&url_cache) {
        if cached_url.trim() == url && path.exists() {
            return Some(path);
        }
    }

    let bytes = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let resp = reqwest::get(&url).await.ok()?;
        resp.bytes().await.ok().map(|b| b.to_vec())
    })
    .await
    .ok()
    .flatten()?;

    tokio::fs::write(&path, &bytes).await.ok()?;
    let _ = std::fs::write(&url_cache, &url);
    Some(path)
}

pub async fn send_command(cmd: LocalCommand) {
    // Determine which app is active
    let active = active_app().await.unwrap_or_else(|| "Music".to_string());
    let script = match cmd {
        LocalCommand::PlayPause => format!("tell application \"{active}\" to playpause"),
        LocalCommand::Next => format!("tell application \"{active}\" to next track"),
        LocalCommand::Prev => format!("tell application \"{active}\" to previous track"),
        LocalCommand::VolumeUp => {
            format!("tell application \"{active}\" to set sound volume to (sound volume + 5)")
        }
        LocalCommand::VolumeDown => {
            format!("tell application \"{active}\" to set sound volume to (sound volume - 5)")
        }
        LocalCommand::Mute => format!("tell application \"{active}\" to set sound volume to 0"),
    };
    let _ = run_osascript(&script).await;
}

async fn active_app() -> Option<String> {
    for app in APPS {
        let script = format!(
            "tell application \"{app}\" to if it is running then get player state as string"
        );
        if let Some(out) = run_osascript(&script).await {
            let s = out.trim().to_lowercase();
            if s == "playing" || s == "paused" {
                return Some(app.to_string());
            }
        }
    }
    None
}

async fn run_osascript(script: &str) -> Option<String> {
    let out = tokio::time::timeout(
        Duration::from_millis(500),
        Command::new("osascript").arg("-e").arg(script).output(),
    )
    .await
    .ok()?
    .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse().ok()
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_meta(
        raw: &str,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    ) {
        let parts: Vec<&str> = raw.splitn(6, "|||").collect();
        if parts.len() < 6 {
            return (None, None, None, None, None, None);
        }
        (
            non_empty(parts[0]),
            non_empty(parts[1]),
            non_empty(parts[2]),
            parse_f64(parts[3]),
            parse_f64(parts[4]),
            parse_f64(parts[5]),
        )
    }

    #[test]
    fn parse_music_meta_output() {
        let raw = "My Song|||The Artist|||My Album|||42.5|||240.0|||75";
        let (title, artist, album, pos, dur, vol) = parse_meta(raw);
        assert_eq!(title.as_deref(), Some("My Song"));
        assert_eq!(artist.as_deref(), Some("The Artist"));
        assert_eq!(album.as_deref(), Some("My Album"));
        assert!((pos.unwrap() - 42.5).abs() < 0.01);
        assert!((dur.unwrap() - 240.0).abs() < 0.01);
        assert!((vol.unwrap() - 75.0).abs() < 0.01);
    }

    #[test]
    fn parse_music_meta_empty_artist() {
        let raw = "Instrumental|||   |||Classical|||10.0|||180.0|||50";
        let (title, artist, _album, _pos, _dur, _vol) = parse_meta(raw);
        assert_eq!(title.as_deref(), Some("Instrumental"));
        assert!(artist.is_none());
    }

    #[test]
    fn parse_f64_handles_comma_locale() {
        assert!((parse_f64("42,5").unwrap() - 42.5).abs() < 0.001);
        assert!((parse_f64("180,0").unwrap() - 180.0).abs() < 0.001);
        assert!((parse_f64("42.5").unwrap() - 42.5).abs() < 0.001);
        assert!(parse_f64("").is_none());
    }
}
