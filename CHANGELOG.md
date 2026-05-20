# Changelog

All notable changes to **ha-tui** are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Per-card overrides** on the `Card` wrapper: `color` (named or `#rrggbb`) and `size` (`small` / `normal` / `large`). `large` renders primary values via `tui-big-text` (centered).
- **Clock card** — local or timezone-aware time with strftime format string.
- **Statistics card** — rolling-window aggregate (avg / min / max / sum / count) of any numeric entity history.
- **MediaPlayer card** — title / artist / app / volume gauge / state, with selection-scoped key bindings (Space, n, p, +, -, m) that call `media_player.*` services.
- **Image card** — renders `image.*` entities and `camera.*` snapshots via `ratatui-image`. Camera cards support `refresh_seconds` for periodic re-fetch. Image-entity cards re-fetch on entity state change. iTerm2 auto-detection forces the iTerm2 protocol where Kitty would render broken; `HA_TUI_IMAGE_PROTO=halfblocks|sixel|kitty|iterm2` overrides.
- **Weather card** — current condition + temperature + humidity + wind + N-day forecast strip. Forecasts fetched via `weather.get_forecasts` (HA 2024+) and re-fetched every 30 minutes.
- **FilteredEntityList `hide_when_empty`** — opt-in: card body skipped when zero matches, grid slot preserved; auto-reappears within ≤250ms when an entity starts matching.

### Changed
- **Graph card** (replaces Sparkline) — multi-series + three render modes:
  - `line` (history-backed, existing behaviour, now N series with legend),
  - `bar` (current value per series, vertical or horizontal),
  - `pie` (current value per series, slice + legend).
  - YAML accepts both new `entities: [...]` and the legacy single `entity:` field; `type: sparkline` still loads via serde alias.
- **Gauge card** rewritten as an HA-style horizontal arc with severity thresholds (`green` / `yellow` / `red`) and an optional needle marker. Old single-bar gauge removed; configs with only `min` / `max` / `unit` keep working.
- **ratatui-image picker** queried before alt-screen entry so high-res protocol detection succeeds (was previously falling back to halfblocks because DCS responses don't round-trip through alt-screen).
- **Card-type picker overlay** now sized to fit all 12 card types with j/k + Enter navigation; digit shortcuts `1`-`9` still work.
- **`tui-big-text`** bumped from 0.7 → 0.8 for ratatui 0.30 compatibility; rendered values are centered horizontally and vertically.

### Documentation
- README rewritten to document all 12 card types, common card fields, terminal requirements, and the image protocol environment variable.
- Spec + implementation plan archived under `docs/superpowers/`.

## [0.1.0] – 2026-05-19

First tagged release. The initial milestones (M0–M5 + partial M6).

### Added
- **Multi-instance HA WebSocket client** built on `tokio-tungstenite` with auth handshake, `subscribe_events`, `call_service`, and exponential-backoff reconnect. One tokio task per instance, mpsc fan-in to the main loop.
- **Entity browser** — live state list, color-prefixed by instance, JSON attributes pane, instance filter cycling.
- **Service calls** — domain-aware default action on `Enter` (light/switch/cover toggle, scripts/scenes turn_on, lock unlock, …). Errors surface in the status bar.
- **YAML dashboards** with 5 card types: `entity`, `toggle`, `gauge`, `sparkline`, `text`. Multiple dashboards switchable via `1`..`9`. `n` creates a new dashboard.
- **EntityList** and **FilteredEntityList** cards (glob + filter query).
- **Interactive dashboard editor** — keyboard (hjkl move, HJKL resize, `a` add, `m` context menu, `d` delete with confirm, `u` undo, `s` save) and full mouse support (click to select, drag to move). Mouse capture enabled.
- **History backfill** — sparkline cards request `history/history_during_period` on connect and append live `state_changed` events.
- **Help overlay** (`?`) and **CLI** flags (`--config`, `--dashboards`).
- **Logging** to a rotating daily file in the XDG data dir.
- **CI** — GitHub Actions matrix for Linux + macOS multi-arch release builds.

### Fixed
- macOS config path uses `~/.config/ha-tui/` (XDG-style) instead of `~/Library/Application Support/...`.
- Explicit raw-mode + alt-screen setup with a panic hook to restore the terminal on crash.
- rustls 0.23 requires an explicit `CryptoProvider` — installed at process start so the first TLS handshake doesn't panic.
- crossterm bumped to 0.29 to match ratatui 0.30.

[Unreleased]: https://github.com/tomstevens/homeassistant-tui/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tomstevens/homeassistant-tui/releases/tag/v0.1.0
