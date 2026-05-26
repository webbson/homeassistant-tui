# Changelog

All notable changes to **ha-tui** are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Removed

## [0.7.2] – 2026-05-26

### Fixed
- Linux release workflow now installs `libdbus-1-dev` before building, fixing CI failures on Ubuntu runners

## [0.7.1] – 2026-05-26

### Changed
- Pre-commit hook now enforces `cargo clippy --all-targets -- -D warnings` in addition to `cargo fmt --check`, so clippy warnings block commits
- `.claude/settings.json` hooks updated to current Claude Code schema

## [0.7.0] – 2026-05-26

### Added
- Update checker now re-checks every 6 hours (was once at startup only)
- Help overlay (`?`) shows the upgrade command (`brew upgrade ha-tui`, `cargo install ha-tui`, or releases URL) when a newer version is available
- Redesigned `media_player` card with optional cover art, artist/album row, prev/play-pause/next controls, progress bar, and vertical volume bar
- New `local_media_player` card type — polls the local machine (macOS Music/Spotify via AppleScript; Linux via MPRIS) with the same rich layout and key controls (space/n/p/+/−)
- `show_cover`, `show_volume`, `show_progress` toggle fields on both `media_player` and `local_media_player` cards (all default `true`)

### Changed

### Fixed
- Cover art no longer flickers on HA state updates — the image protocol is now reused when the cached `media_content_id` already matches the current track; a new `StatefulProtocol` (which forces a full image retransmit) is only created on genuine track changes. Failed fetches insert a sentinel to stop infinite render-time retries.
- App events (HA state pushes, image bytes, local media updates) no longer trigger immediate redraws; they are rendered on the next 250 ms tick, preventing iTerm2/sixel protocols from retransmitting cover art on every Spotify position-update event.
- Spotify progress bar duration is now correct — AppleScript `duration of track` returns milliseconds for Spotify (seconds for Music.app); the `/1000` conversion is now applied to Spotify duration only. `player position` and `sound volume` wrapped in `try` blocks so unsupported properties don't abort the whole script.
- Cover art is now centered in a square sub-rect so it doesn't appear at top-left in landscape card areas
- Mouse left-click on ⏮/⏵/⏭ controls dispatches the correct media command; click zones are centered on the 7-cell control block rather than splitting the full card width into thirds
- Local media cover art now reloads on track change — Spotify reuses the same file path so path comparison alone never triggered a reload
- Progress bar position now interpolates from `media_position_updated_at` so it advances smoothly between HA state pushes

### Removed

## [0.6.0] – 2026-05-25

### Added
- **Instance management UI**: press `i` to open the instance list, then `a` add / `e` edit / `d` delete an instance — no more editing YAML by hand and restarting.
- Instance form: Host/IP field + SSL checkbox — the full `ws(s)://host/api/websocket` URL is built automatically on save.
- Name field is optional; auto-derives from the first hostname label when left blank.
- Token field is masked with `•`; in edit mode an empty token preserves the existing value.
- On first launch with no config file the add-instance modal opens automatically; Esc quits.
- Rename cascade: renaming an instance rewrites all card `instance:` references in `dashboards.yaml` automatically.
- Delete guard: deleting an instance shows how many cards will be removed before confirming.
- Config persisted on every add/edit/delete so changes survive restart.

### Changed
- First-run bootstrap writes a minimal `instances: []` stub config instead of the example file.
- Token round-trip: raw `${ENV_VAR}` / `token_file:` forms are preserved on save; only the resolved value is used at runtime.

## [0.5.4] – 2026-05-22

### Changed
- Grid `preferred_height` now honors `CardSize::Large` (minimum 6 rows so `big_text` glyphs always fit) and `CardSize::Small` for Weather/MediaPlayer (3 rows). Previously the size setting was a no-op on grid layouts because cards had no fixed height and big-text never fit.

### Fixed
- Grid editor: `FilteredEntityList` cards with `hide_when_empty: true` now stay visible as a dimmed placeholder when empty, so they can still be selected and edited. Hidden cards no longer disappear from the editor.
- Grid editor: the yellow selection border now matches the card's actual rendered height. The overlay used a fixed dummy height of 4 and ignored dynamic entity counts / size settings, drawing the selection box in the wrong place.

## [0.5.3] – 2026-05-22

### Added
- Release pipeline via `cargo-dist`: tagged pushes (`v*`) build macOS (arm64/x64) and Linux (x64/arm64) binaries, attach them to a GitHub Release, and publish a Homebrew formula to `webbson/homebrew-tap`. Install with `brew install webbson/tap/ha-tui` or the shell installer linked from the release page.

### Changed
- `ci.yml` now runs only test/clippy/fmt on `main` pushes and pull requests; release artifacts are produced by the new `release.yml` (cargo-dist) on tag pushes. The previous Windows build target was dropped — Windows is no longer published.

### Removed
- Windows build target and Windows terminal documentation. Re-add by appending `x86_64-pc-windows-msvc` to `workspace.metadata.dist.targets` in `Cargo.toml`.

## [0.5.2] – 2026-05-21

### Fixed
- Grid layout: `attribute_list` card now computes preferred height from actual array length instead of the static default of 4 rows
- Grid layout: `image` and `graph` cards in a non-fill column now expand to absorb any remaining column space

## [0.5.1] – 2026-05-21

### Added
- **Grid dashboard creation prompts for first row** — creating a new grid-layout dashboard immediately opens the row-height picker so the user isn't left with a blank, unnavigable canvas.

### Changed
- **Image card fills full card area** — switched from `Resize::Fit` (letterboxed) to `Resize::Scale` so the image scales to fill the entire card in grid layout.

### Fixed

### Removed

## [0.5.0] – 2026-05-21

### Added
- **AttributeList card** — renders an entity attribute array (e.g. `top10`) as a scrollable list with a configurable per-row template. Supports `{key}`, `{key|round}`, `{key|int}`, `{key|thousands}`, `{rank}`, and `{index}` tokens. Optional `limit` caps the number of rows shown. Add via the editor palette (`a` → "attribute list") or directly in YAML.
- **Cursor movement in editor text inputs** — Left/Right/Home/End/Delete move the cursor within any editor text buffer. Ctrl+A/E jump to start/end; Ctrl+W deletes the word before the cursor.

### Changed

### Fixed
- **FilteredEntityList with `hide_when_empty` collapses to zero height when empty** — the card no longer occupies space in grid-layout columns when it has no matching entities.
- **EntityList edit preserves title and per-entry overrides** — "Change entities" on an existing EntityList card no longer clears the card title or drops `name`/`hide_state` overrides set on individual entries.

### Removed

## [0.4.0] – 2026-05-21

### Added
- **Entity details overlay** — press `D` on any dashboard card to open a fullscreen popup showing entity_id, state, last_changed, last_updated, and full attributes JSON. `j`/`k` scroll, `PgUp`/`PgDn` page, `Esc`/`q`/`D` close.
- **Graph legend current value** — line, bar, and pie graph cards show the current formatted state value (with unit) next to each series name in the legend.
- **EntityList per-entry overrides** — items accept `{entity, name?, hide_state?}` form for per-entity display overrides; bare strings still supported and round-trip unchanged.
- **FilteredEntityList overrides** — accepts `overrides: { <entity_id>: { name?, hide_state? } }` for per-entity display overrides.
- **Editor entry overrides** — "Set entry name" and "Toggle hide state" menu items for EntityList/FilteredEntityList row overrides.
- Enter on `button.*` and `input_button.*` entities calls the `press` service.
- Enter on `input_number`, `input_text`, `input_select`, or `input_datetime` entities opens an input modal with attribute-driven validation (min/max for numbers, length limits for text, option list for select, ISO date/time format for datetime). `↑`/`↓` adjusts numbers by step or navigates select options; `Esc` cancels.

### Fixed
- Lock entities now toggle correctly: `unlock` when locked, `lock` when unlocked. Unknown state still calls `unlock` (legacy behavior preserved).

### Changed
- Entity/EntityList/Gauge value formatting respects `display_precision`; when absent, whole numbers drop decimals (`21.0` → `21`).
- Graph bar/pie charts use `BarChart`/`tui-piechart` ratatui widgets (previously hand-drawn with Canvas). Pie slices are filled; bar widths scale to card width.
- All dim/secondary text uses `Style::new().dim()` instead of hardcoded `Color::DarkGray`, making the UI readable on both dark and light terminal themes.

### Removed

## [0.3.0] – 2026-05-20

### Added
- **Grid layout** — new `layout: type: grid` dashboard type. Dashboards are a vertical stack of rows; each row has one or more columns of cards. Cards stack vertically at natural height; columns scroll when content overflows. Rows support `height: <n>` (fixed terminal rows) or `height: auto` (shares remaining space). Columns and rows support `fill_height` to scale card heights proportionally to fill the column.
- **Grid editor** — full structural editing in the grid layout: add/remove/reorder rows and columns, move cards up/down within a column, set row heights from the menu. `R` focuses the row menu, `C` the column menu, `Esc` returns to card focus. Row/column menus accessible via `m`.
- **Cross-dashboard move/copy** — `Move to dashboard…` and `Copy to dashboard…` in the card menu for both Free and Grid dashboards. Grid targets prompt for destination row and column.
- **Stable `CardId`** — cards carry a persistent numeric ID assigned at load. Selection is stable across structural mutations (reordering rows/columns does not change which card is highlighted).

### Changed

### Fixed

### Removed

## [0.2.0] – 2026-05-20

### Added
- **First-run bootstrap** — missing `~/.config/ha-tui/config.yaml` is created from the bundled example; missing `dashboards.yaml` gets a one-card Welcome dashboard with setup instructions, so a fresh install lands on a usable screen.
- **Entity / instance modals** — `E` and `i` now open a centered, searchable overlay on top of the current dashboard instead of switching to a full screen. `Esc` closes back.
- **Update notifier** — on startup, asynchronously checks GitHub for a newer release of `webbson/homeassistant-tui` and shows a yellow `↑ vX.Y.Z` in the header when available. Failures are silent.
- **Per-card overrides** on the `Card` wrapper: `color` (named or `#rrggbb`) and `size` (`small` / `normal` / `large`). `large` renders primary values via `tui-big-text` (centered).
- **Clock card** — local or timezone-aware time with strftime format string.
- **Statistics card** — rolling-window aggregate (avg / min / max / sum / count) of any numeric entity history.
- **MediaPlayer card** — title / artist / app / volume gauge / state, with selection-scoped key bindings (Space, n, p, +, -, m) that call `media_player.*` services.
- **Image card** — renders `image.*` entities and `camera.*` snapshots via `ratatui-image`. Camera cards support `refresh_seconds` for periodic re-fetch. Image-entity cards re-fetch on entity state change. iTerm2 auto-detection forces the iTerm2 protocol where Kitty would render broken; `HA_TUI_IMAGE_PROTO=halfblocks|sixel|kitty|iterm2` overrides.
- **Weather card** — current condition + temperature + humidity + wind + N-day forecast strip. Forecasts fetched via `weather.get_forecasts` (HA 2024+) and re-fetched every 30 minutes.
- **FilteredEntityList `hide_when_empty`** — opt-in: card body skipped when zero matches, grid slot preserved; auto-reappears within ≤250ms when an entity starts matching.

### Changed
- **Default screen is Dashboard** (was Entities). The Entities/Instances full-screen views were removed in favor of modal overlays.
- **Dashboard header redesigned** — dashboard name is centered on the existing top row; the separate `◆ name` body row is gone. In edit mode the row gains a `✎ col,row` pen indicator on the right; the standalone `editing: …` banner row is removed.
- **Footer trimmed** on the Dashboard screen — the `dashboard: name` prefix is dropped (now in the header) and the `q quit` hint is removed.
- **Graph card** (replaces Sparkline) — multi-series + three render modes:
  - `line` (history-backed, existing behaviour, now N series with legend),
  - `bar` (current value per series, vertical or horizontal),
  - `pie` (current value per series, slice + legend).
  - YAML accepts both new `entities: [...]` and the legacy single `entity:` field; `type: sparkline` still loads via serde alias.
- **Gauge card** rewritten as an HA-style horizontal arc with severity thresholds (`green` / `yellow` / `red`) and an optional needle marker. Old single-bar gauge removed; configs with only `min` / `max` / `unit` keep working.
- **ratatui-image picker** queried before alt-screen entry so high-res protocol detection succeeds (was previously falling back to halfblocks because DCS responses don't round-trip through alt-screen).
- **Card-type picker overlay** now sized to fit all 12 card types with j/k + Enter navigation; digit shortcuts `1`-`9` still work.
- **`tui-big-text`** bumped from 0.7 → 0.8 for ratatui 0.30 compatibility; rendered values are centered horizontally and vertically.

### Removed
- **`q` quit binding** — `Esc` is the only way to quit. Inside an overlay or the editor, `Esc` first closes the overlay or exits the editor before quitting on a second press from the dashboard.

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

[Unreleased]: https://github.com/webbson/homeassistant-tui/compare/v0.7.2...HEAD
[0.7.2]: https://github.com/webbson/homeassistant-tui/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/webbson/homeassistant-tui/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/webbson/homeassistant-tui/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/webbson/homeassistant-tui/compare/v0.5.4...v0.6.0
[0.5.4]: https://github.com/webbson/homeassistant-tui/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/webbson/homeassistant-tui/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/webbson/homeassistant-tui/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/webbson/homeassistant-tui/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/webbson/homeassistant-tui/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/webbson/homeassistant-tui/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/webbson/homeassistant-tui/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/webbson/homeassistant-tui/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/webbson/homeassistant-tui/releases/tag/v0.1.0
