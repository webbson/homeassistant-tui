use super::{LocalCommand, LocalMediaSnapshot};

pub async fn fetch_snapshot() -> LocalMediaSnapshot {
    tokio::task::spawn_blocking(fetch_blocking)
        .await
        .unwrap_or_default()
}

fn fetch_blocking() -> LocalMediaSnapshot {
    let Ok(finder) = mpris::PlayerFinder::new() else {
        return LocalMediaSnapshot::default();
    };
    let Ok(player) = finder.find_active() else {
        return LocalMediaSnapshot::default();
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

    let art_path = meta.as_ref().and_then(|m| {
        let url = m.art_url()?;
        url.strip_prefix("file://").map(std::path::PathBuf::from)
    });

    let position_secs = player.get_position().ok().map(|d| d.as_secs_f64());

    let duration_secs = meta
        .as_ref()
        .and_then(|m| m.length().map(|d| d.as_secs_f64()));

    let volume_0_1 = player.get_volume().ok();

    let player_name = player.identity().to_string();

    LocalMediaSnapshot {
        player_name,
        title,
        artist,
        album,
        position_secs,
        duration_secs,
        volume_0_1,
        is_muted: false,
        is_playing,
        art_path,
    }
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
