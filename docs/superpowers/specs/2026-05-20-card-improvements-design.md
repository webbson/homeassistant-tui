# Card improvements & new card types — design

**Date:** 2026-05-20
**Status:** Approved (pending implementation plan)
**Author:** Tom Stevens + Claude
**Scope:** ha-tui dashboard cards — uniform per-card overrides, FilteredEntityList visibility, gauge replacement, sparkline rename, and six new card types.

---

## 1. Motivation

Current dashboard cards are limited:

- No way to override instance color on a per-card basis (a single instance with many entity types renders monochrome).
- No way to vary visual emphasis between cards — every entity card looks identical regardless of importance.
- `FilteredEntityList` always renders its frame even when zero entities match, leaving dead space on dashboards.
- The current `Gauge` card is a thin vertical-bar wrapper that doesn't match what Home Assistant users expect from a gauge.
- "Sparkline" is misnamed — the widget renders full axes and grid lines via ratatui's `Chart`, not a sparkline.
- Several Home Assistant-equivalent cards have no analog (clock, weather, image/camera, media player, statistics).

This spec covers a coordinated set of changes that touch most card types and add new ones.

## 2. Goals & non-goals

**Goals**

- Add uniform per-card `color` and `size` overrides applied through the `Card` wrapper.
- Hide `FilteredEntityList` content (but keep grid slot) when no entities match, opt-in.
- Replace the existing `Gauge` card with a Home Assistant-style gauge (severity zones + needle).
- Rename `Sparkline` → `Graph` with serde alias for backwards compatibility.
- Add new cards: `Clock`, `Weather`, `Image`, `MediaPlayer`, `Statistics`.

**Non-goals**

- Map/device-tracker visualisation.
- Energy dashboard.
- Logbook card.
- Conditional cards (covered indirectly by `hide_when_empty`).
- Alarm control panel card.
- Editor-side drag-to-resize of new cards beyond the existing `HJKL` flow.

## 3. Common card fields

Add two optional fields to the `Card` wrapper struct (not to each `CardKind` variant) so they apply uniformly:

```rust
pub struct Card {
    #[serde(flatten)]
    pub kind: CardKind,
    pub pos: Pos,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub size: Option<CardSize>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardSize {
    Small,
    #[default]
    Normal,
    Large,
}
```

### 3.1 `color`

- Accepts named colour (`red`, `green`, `blue`, …) or `#rrggbb` hex.
- Parsing reuses the existing instance-colour parser in `ui::theme`.
- When `None`, falls back to the instance colour (existing behaviour).
- For cards with no instance (`Text`, `Clock`), `color` overrides the default text colour; when `None` they use the theme default.

### 3.2 `size`

- Affects only the card's *content area*; border, title bar, and grid footprint are unchanged.
- `Normal` (default): existing rendering.
- `Small`: reduce internal padding, drop secondary labels (e.g. unit suffix shown only inline, no separate line).
- `Large`: render the primary value via `tui-big-text` (block-glyph numerals/letters). Applies to `Entity`, `Toggle`, `Gauge`, `Clock`, `Statistics`. Other card types ignore `Large` and fall back to `Normal`.

Add new dep: `tui-big-text = "0.7"` (or current latest compatible with ratatui 0.30).

## 4. FilteredEntityList: `hide_when_empty`

```rust
FilteredEntityList {
    instance: Alias,
    query: String,
    #[serde(default)] hide_state: bool,
    #[serde(default)] hide_when_empty: bool,
    #[serde(default)] title: Option<String>,
}
```

When `hide_when_empty: true` and the query matches zero entities at render time:

- Skip drawing the border, title, and contents.
- The card still owns its grid cells (no reflow of neighbouring cards).
- In editor mode, the card is still selectable and shows a placeholder outline so the user can edit/move/delete it.

## 5. Gauge card replacement

The existing `CardKind::Gauge` is removed and replaced with a new variant under the same YAML key (`type: gauge`). Existing YAML using only `min`, `max`, `unit` continues to parse — additional fields are optional.

```rust
Gauge {
    instance: Alias,
    entity: EntityId,
    min: f64,
    max: f64,
    #[serde(default)] unit: Option<String>,
    #[serde(default)] severity: Option<Severity>,
    #[serde(default = "default_true")] needle: bool,
    #[serde(default)] title: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Severity {
    pub green: f64,
    pub yellow: f64,
    pub red: f64,
}
```

### 5.1 Rendering

- Horizontal arc drawn with unicode block characters (`▁▂▃▄▅▆▇█`) across the card's content width.
- When `severity` is present, segments left of `green` render in theme green, between `green` and `yellow` in yellow, ≥ `red` in red. Thresholds are interpreted as values on the `min..=max` scale.
- When `severity` is absent, arc renders in the card's resolved colour (instance or override).
- When `needle: true`, the cell representing the current value is rendered with inverted foreground/background.
- The numeric value + unit is shown beneath the arc (or inside the arc when `size: large`).

### 5.2 Migration

Old configs with just `min`/`max`/`unit` parse correctly. `severity` and `needle` default to `None` / `true` so behaviour for existing dashboards is "horizontal arc in instance colour with needle". Visual change vs. the old vertical bar is documented in the release notes.

## 6. Sparkline → Graph rename

```rust
#[serde(rename = "graph", alias = "sparkline")]
Graph {
    instance: Alias,
    entity: EntityId,
    #[serde(default = "default_window")] window: String,
    #[serde(default)] title: Option<String>,
}
```

- Editor palette entry renamed `Graph`.
- Help overlay updated.
- Internal type name `CardKind::Graph` (was `Sparkline`).
- README + examples updated to use `graph`.
- Old YAML using `type: sparkline` continues to load via the serde alias.

## 7. New card: Clock

```rust
Clock {
    #[serde(default = "default_clock_format")] format: String,
    #[serde(default)] timezone: Option<String>,
    #[serde(default)] title: Option<String>,
}

fn default_clock_format() -> String { "%H:%M:%S".into() }
```

- Renders the current time formatted via `chrono` (already a dependency).
- `timezone` is an IANA name (`Europe/Stockholm`); requires new dep `chrono-tz = "0.10"`. When `None`, uses system local time.
- Refresh: the existing 250ms tick already drives a redraw; no new timer needed.
- `size: large` renders the time via `tui-big-text`.

## 8. New card: Weather

```rust
Weather {
    instance: Alias,
    entity: EntityId,                          // weather.* domain
    #[serde(default = "default_true")] show_forecast: bool,
    #[serde(default = "default_forecast_days")] forecast_days: u8,
    #[serde(default)] title: Option<String>,
}

fn default_forecast_days() -> u8 { 3 }
```

- Reads the weather entity's `state` (current condition string) and attributes (`temperature`, `humidity`, `wind_speed`, `wind_bearing`).
- Current condition maps to a unicode glyph (table of `sunny`/`cloudy`/`partlycloudy`/`rainy`/`snowy`/`fog`/`lightning`/…).
- Forecast: Home Assistant moved forecasts off the entity state in 2024. Need a new `HaCommand::GetWeatherForecast { instance, entity, kind: Daily }` that fires `weather.get_forecasts` via the WS `call_service` flow, returning the response array. Forecasts are cached on the instance state keyed by entity id; cards re-render when the cache updates. Re-fetch every 30 min and on app start.
- Layout (default `Normal`): top row current temp + glyph + condition text; second row humidity / wind; remaining rows forecast (per-day glyph + hi/lo).
- `size: large`: only current temp + condition glyph; no forecast.
- `size: small`: only current temp + glyph, no extra attributes.

## 9. New card: Image

```rust
Image {
    instance: Alias,
    source: ImageSource,
    #[serde(default)] refresh_seconds: Option<u32>,
    #[serde(default)] title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageSource {
    ImageEntity { entity: EntityId },   // image.* domain
    Camera { entity: EntityId },        // camera.* domain
}
```

### 9.1 Dependencies

- New: `ratatui-image = "8"` (current latest compatible with ratatui 0.30 — version verified during implementation).
- New: `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls-native-roots", "stream"] }` for fetching image bytes. Reuses the system's native root certs already used by tokio-tungstenite.
- New: `image = "0.25"` (required transitively by `ratatui-image` for decoding).

### 9.2 Fetching

- Image entities: HTTP GET `<base_url>/api/image_proxy/<entity_id>?token=<long_lived_token>` (or with `Authorization: Bearer <token>` header — chosen at implementation time based on whichever the HA REST API currently accepts for image_proxy). `<base_url>` is derived from the existing WS URL (`wss://host:port` → `https://host:port`).
- Camera entities: HTTP GET `<base_url>/api/camera_proxy/<entity_id>` with the same auth.
- For `ImageEntity`, fetch on first render and when the entity state changes (state for `image.*` is a timestamp of last update).
- For `Camera`, if `refresh_seconds` is set, spawn a per-card tokio interval that emits `AppEvent::FetchImage { card_id }`. App-level dispatcher dedups and routes through a new `HaCommand::FetchImageBytes`. Without `refresh_seconds`, fetch once on initial render.

### 9.3 Rendering

- Decoded image is held by `ratatui-image::picker::Picker` (auto-detects sixel/kitty/iterm2/halfblocks based on terminal capabilities at app startup).
- A per-card `StatefulProtocol` is stored on the dashboard state alongside the card position so re-renders are cheap.
- Card holds the most recent decoded image; while a new fetch is in flight, the previous frame stays on screen.
- If the terminal supports no image protocol, fall back to halfblocks (always supported).
- Failed fetches show "Image unavailable" inside the card border with the last error in the status bar.

### 9.4 Editor support

- New entity-picker filter: when adding an `Image` card, the entity prompt only shows `image.*` and `camera.*` entities. `refresh_seconds` is prompted when `Camera` is chosen.

## 10. New card: MediaPlayer

```rust
MediaPlayer {
    instance: Alias,
    entity: EntityId,            // media_player.* domain
    #[serde(default)] title: Option<String>,
}
```

- Displays now-playing: `media_title`, `media_artist`, source/app name, volume (as a bar), state (playing/paused/idle/off).
- Keyboard interactions when card is selected on a dashboard:
  - `Space` → `media_play_pause`
  - `n` → `media_next_track`
  - `p` → `media_previous_track`
  - `+` / `-` → `volume_up` / `volume_down`
  - `m` → `volume_mute` toggle
- These bindings are dashboard-scoped, only active when the selected card is a `MediaPlayer`. They override generic dashboard keys only for the duration of selection — escape resets focus.
- Visuals scale with `size`: `Small` shows title + state only; `Large` adds large-text title.

## 11. New card: Statistics

```rust
Statistics {
    instance: Alias,
    entity: EntityId,
    #[serde(default = "default_window")] window: String,
    metric: StatsMetric,
    #[serde(default)] unit: Option<String>,
    #[serde(default)] title: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatsMetric { Avg, Min, Max, Sum, Count }
```

- Computes the metric from the same history backing the `Graph` card (`util::history::RingBuf`).
- On `HaInitialStates`, each `Statistics` card triggers a `FetchHistory` for its window (already implemented for graphs — reuse).
- Live `state_changed` events update the buffer; the metric is recomputed on each redraw (cheap — buffer cap is 8192).
- Renders the metric value + unit, with the metric name + window as a subtitle (e.g. `avg · 24h`).
- `size: large` uses big-text for the metric value.

## 12. Editor & menu changes

- Card add-menu entries:
  - Existing: Entity, Toggle, Gauge, Graph (renamed), Text, EntityList, FilteredEntityList.
  - New: Clock, Weather, Image, MediaPlayer, Statistics.
- Card context-menu (`m` on a selected card) gains two entries:
  - "Color override" → opens a colour input (named or `#rrggbb`, blank to clear).
  - "Size" → cycle Small/Normal/Large.
- FilteredEntityList context menu gains "Hide when empty" toggle.
- Gauge context menu gains "Severity thresholds" (3 numeric prompts) and "Needle" toggle.
- All new cards follow the existing per-card add flow (type → instance → entity → fine-tune).

## 13. Persistence

- `serde_yaml` round-trip is preserved for all new fields. Default-valued fields use `#[serde(skip_serializing_if = "...")]` to avoid bloating YAML files with `null`s and `false`s.
- No migration step is needed: old YAML loads via serde defaults + aliases.

## 14. Dependencies summary

Added:

- `tui-big-text` — large block-glyph text.
- `chrono-tz` — IANA timezone support for `Clock`.
- `ratatui-image` — image protocol rendering.
- `reqwest` (rustls-tls-native-roots, stream) — HTTP client for HA image/camera proxies.
- `image` — pulled in by `ratatui-image`, listed for clarity.

Removed: none.

## 15. Testing

- Unit tests:
  - `Card` color/size serde round-trip with and without overrides.
  - `Severity` parsing and threshold→colour mapping.
  - Sparkline serde alias loads as `Graph`.
  - `FilteredEntityList { hide_when_empty: true }` with empty query result returns `None` from the render path.
  - Statistics: avg/min/max/sum/count over a known `RingBuf`.
- Snapshot tests (`insta`):
  - One snapshot per new card type at `size: normal` with synthetic state.
  - Gauge with and without severity zones.
- Integration:
  - Image card defers to actual binary fetch; gate behind `#[cfg(feature = "real-ha")]` or skip in unit suite. A pure-Rust stub server in tests is out of scope.

## 16. Implementation milestones

1. **M1**: `Card.color` + `Card.size` fields + plumbing. Apply in all existing renderers. New menu entries.
2. **M2**: Sparkline → Graph rename. FilteredEntityList `hide_when_empty`.
3. **M3**: Replace old Gauge with new HA-style Gauge.
4. **M4**: Clock card.
5. **M5**: Statistics card (reuses existing history plumbing — small).
6. **M6**: MediaPlayer card (new key bindings, service calls).
7. **M7**: Image card (largest — new deps, fetch loop, ratatui-image integration).
8. **M8**: Weather card (forecast fetch + cache).

Each milestone lands on its own feature branch, tested via `cargo test` and `cargo clippy --all-targets`, with insta snapshots reviewed before merge.

## 17. Risks & open questions

- **`ratatui-image` 0.30 compatibility** — the crate's MSRV and ratatui version pin must be verified at implementation time. If incompatible, fall back to halfblocks-only via a hand-rolled rendering path.
- **HA image_proxy auth scheme** — the long-lived token may need to be passed as a query param vs. a header depending on HA version. Implementation will test both and pick the working one.
- **Weather forecast service shape** — HA's `weather.get_forecasts` service response shape may differ across releases. Pin to the 2024.x shape and document the minimum supported HA version.
- **MediaPlayer key binding conflicts** — `Space` and `n`/`p` are unused in dashboard navigation today, but verify before landing.
- **Large card size on small terminals** — `size: large` may overflow narrow cards. Renderer falls back to `Normal` when the card's content area is smaller than the big-text minimum.
