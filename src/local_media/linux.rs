use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use super::{LocalCommand, LocalMediaSnapshot};

pub async fn fetch_snapshot() -> LocalMediaSnapshot {
    let (mut snapshot, art_url) = tokio::task::spawn_blocking(fetch_blocking)
        .await
        .unwrap_or_default();
    if let Some(raw) = art_url {
        snapshot.art_path = resolve_art_url(&raw).await;
    }
    snapshot
}

/// Returns the snapshot (art_path = None) and the raw artUrl separately so the
/// async resolver can run outside spawn_blocking.
fn fetch_blocking() -> (LocalMediaSnapshot, Option<String>) {
    let Ok(finder) = mpris::PlayerFinder::new() else {
        return (LocalMediaSnapshot::default(), None);
    };
    let Ok(player) = finder.find_active() else {
        return (LocalMediaSnapshot::default(), None);
    };

    let is_playing = player
        .get_playback_status()
        .map(|s| s == mpris::PlaybackStatus::Playing)
        .unwrap_or(false);

    let meta = player.get_metadata().ok();

    let title = meta.as_ref().and_then(|m| m.title().map(str::to_string));
    let artist = meta
        .as_ref()
        .and_then(|m| m.artists().and_then(|v| v.first().map(|s| s.to_string())));
    let album = meta
        .as_ref()
        .and_then(|m| m.album_name().map(str::to_string));
    let art_url = meta.as_ref().and_then(|m| m.art_url().map(str::to_string));

    let position_secs = player.get_position().ok().map(|d| d.as_secs_f64());

    let duration_secs = meta
        .as_ref()
        .and_then(|m| m.length().map(|d| d.as_secs_f64()));

    let volume_0_1 = player.get_volume().ok();

    let player_name = player.identity().to_string();

    let snapshot = LocalMediaSnapshot {
        player_name,
        title,
        artist,
        album,
        position_secs,
        duration_secs,
        volume_0_1,
        is_muted: false,
        is_playing,
        art_path: None,
    };
    (snapshot, art_url)
}

/// Resolve a raw MPRIS artUrl to a local PathBuf suitable for `tokio::fs::read`.
///
/// `file://` paths are percent-decoded via the `url` crate. `http(s)://` URLs
/// are downloaded to $TMPDIR with a sidecar cache file to skip re-downloads on
/// repeated polls for the same URL.
async fn resolve_art_url(raw: &str) -> Option<PathBuf> {
    use url::Url;

    let parsed = match Url::parse(raw) {
        Ok(u) => u,
        Err(_) => {
            tracing::warn!(raw, "mpris artUrl is not a valid URL, skipping cover art");
            return None;
        }
    };

    match parsed.scheme() {
        "file" => parsed.to_file_path().ok().or_else(|| {
            tracing::warn!(raw, "mpris artUrl file:// path could not be decoded");
            None
        }),
        "http" | "https" => download_remote_art(raw).await,
        scheme => {
            tracing::warn!(
                scheme,
                raw,
                "mpris artUrl scheme unsupported, skipping cover art"
            );
            None
        }
    }
}

async fn download_remote_art(url: &str) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();

    let path = std::env::temp_dir().join(format!("ha-tui-mpris-art-{hash:016x}.img"));
    let url_cache = std::env::temp_dir().join(format!("ha-tui-mpris-art-{hash:016x}.url"));

    if let Ok(cached_url) = std::fs::read_to_string(&url_cache) {
        if cached_url.trim() == url && path.exists() {
            return Some(path);
        }
    }

    let bytes = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let resp = reqwest::get(url).await.ok()?;
        resp.bytes().await.ok().map(|b| b.to_vec())
    })
    .await
    .ok()
    .flatten()?;

    tokio::fs::write(&path, &bytes).await.ok()?;
    let _ = std::fs::write(&url_cache, url);
    Some(path)
}

pub async fn send_command(cmd: LocalCommand) {
    tokio::task::spawn_blocking(move || send_command_blocking(cmd))
        .await
        .ok();
}

fn send_command_blocking(cmd: LocalCommand) {
    let Ok(finder) = mpris::PlayerFinder::new() else {
        return;
    };
    let Ok(player) = finder.find_active() else {
        return;
    };
    let _ = match cmd {
        LocalCommand::PlayPause => player.play_pause(),
        LocalCommand::Next => player.next(),
        LocalCommand::Prev => player.previous(),
        LocalCommand::VolumeUp => {
            let vol = player.get_volume().unwrap_or(0.5);
            player.set_volume((vol + 0.05).min(1.0))
        }
        LocalCommand::VolumeDown => {
            let vol = player.get_volume().unwrap_or(0.5);
            player.set_volume((vol - 0.05).max(0.0))
        }
        LocalCommand::Mute => player.set_volume(0.0),
    };
}
