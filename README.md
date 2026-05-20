# ha-tui

Terminal UI for [Home Assistant](https://www.home-assistant.io/). Multi-instance, color-coded entities, YAML-defined dashboards, interactive dashboard editor with **mouse + keyboard**. Written in Rust on top of [Ratatui](https://ratatui.rs).

## Features

- Connect to multiple HA instances at once over WebSocket (long-lived access tokens).
- Color-code every entity by the instance it came from.
- Browse entities live; trigger domain-aware default actions with Enter (toggle lights / switches / covers, run scripts, etc.).
- Compose dashboards in YAML with 12 card types (see below).
- Interactive dashboard editor: cursor + selection model, undo/redo, save back to YAML. Works with keyboard **or** mouse.
- Logs to `~/.local/share/ha-tui/log/` (XDG-aware) so terminal output stays clean.

## Install

```bash
# Rust toolchain (one-time)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build --release
# Optional: install to ~/.cargo/bin
cargo install --path .
```

## Configure

On first run, ha-tui auto-creates `~/.config/ha-tui/config.yaml` (from the bundled example) and `~/.config/ha-tui/dashboards.yaml` (a one-card Welcome dashboard) if they don't already exist. You only need to copy them manually if you want to set things up before launching:

```bash
mkdir -p ~/.config/ha-tui
cp config/config.example.yaml      ~/.config/ha-tui/config.yaml
cp config/dashboards.example.yaml  ~/.config/ha-tui/dashboards.yaml
```

Edit `config.yaml`:

```yaml
instances:
  - alias: home
    url: "wss://ha.example.com/api/websocket"
    token: "${HA_HOME_TOKEN}"     # env-expand at load
  - alias: cabin
    url: "ws://192.168.50.10:8123/api/websocket"
    token_file: "~/.config/ha-tui/cabin.token"
    color: "yellow"               # optional override; otherwise auto-assigned
log_level: "info"
```

Tokens are created in HA at **Profile → Long-Lived Access Tokens**.

## Run

```bash
ha-tui
# or
ha-tui --config /path/to/config.yaml --dashboards /path/to/dashboards.yaml
```

## Keys

### Global

| Key | Action |
|---|---|
| `Esc` | Quit (or close current overlay/editor first) |
| `E` | Entity search modal |
| `i` | Instance list modal |
| `1`..`9` | Jump to dashboard N |
| `e` | Edit the currently shown dashboard |
| `?` | Help overlay |

### Entity search modal

| Key | Action |
|---|---|
| type | Fuzzy filter on `entity_id` |
| `↓`/`↑` or `PageUp`/`PageDown` | Navigate |
| `f` | Cycle instance filter (all → first → next → …) |
| `Enter` | Domain-default action (toggle lights/switches, turn_on scripts/scenes, etc.) |
| `Esc` | Close modal |

### Dashboard editor

| Key | Action |
|---|---|
| `hjkl` / arrows | Move cursor |
| `Enter` (no selection) | Select card under cursor |
| `Enter` (with selection) | Move selected card to cursor; deselect |
| `HJKL` | Resize selected card (left/down/up/right) |
| `a` | Add card (menu-driven type picker, then entity/options) |
| `m` | Context menu for selected card or dashboard |
| `d` | Delete selected card (confirm) |
| `u` | Undo |
| `s` | Save dashboard YAML |
| `Esc` | Exit editor (confirm if unsaved) |

Mouse in editor:
- Click a cell → select the card there.
- Click + drag a card → move it.
- Mouse cursor also drives the cursor cell — keyboard and mouse share state.

## Dashboard YAML

`pos` units are grid cells, not characters. `grid` defaults to `{ cols: 12, rows: 24 }`.

### Common card fields

Every card supports two optional fields on the `Card` wrapper:

| Field   | Type                         | Effect                                                                                                    |
|---------|------------------------------|-----------------------------------------------------------------------------------------------------------|
| `color` | `String` (named or `#rrggbb`) | Overrides the instance-derived border + value colour for this card.                                       |
| `size`  | `small` / `normal` / `large` | Affects the content area only. `large` renders primary values (state / temp / time) via `tui-big-text`.   |

```yaml
- type: entity
  instance: home
  entity: light.kitchen
  color: "#ff8800"
  size: large
  pos: { col: 0, row: 0, w: 6, h: 4 }
```

In the editor, press `m` on a selected card to set these via overlays.

### entity

Displays state of any HA entity with optional live ticker animation.

```yaml
- type: entity
  instance: home
  entity: light.kitchen
  title: "Kitchen Light"          # optional; defaults to friendly name
  ticker: true                    # optional scroll animation
  pos: { col: 0, row: 0, w: 4, h: 2 }
```

### toggle

On/off indicator card.

```yaml
- type: toggle
  instance: home
  entity: switch.coffee
  pos: { col: 4, row: 0, w: 3, h: 2 }
```

### gauge

HA-style horizontal arc with severity thresholds and optional needle.

```yaml
- type: gauge
  instance: cabin
  entity: sensor.cabin_temp
  min: -20
  max: 30
  unit: "°C"
  needle: true                    # optional; default true
  severity:                       # optional thresholds — values below `yellow`
    green: -20                    # render green, ≥ yellow render yellow,
    yellow: 10                    # ≥ red render red
    red: 22
  pos: { col: 0, row: 2, w: 4, h: 3 }
```

### graph

Multi-series chart card. Three render modes: `line` (history-backed), `bar` (current value per series), `pie` (current value per series).

```yaml
- type: graph
  instance: home
  graph_type: line                # line | bar | pie
  window: "1h"                    # Line only — history window: 1h, 6h, 24h, 7d
  orientation: vertical           # Bar only — vertical | horizontal
  entities:
    - { entity: sensor.power_in,  label: "In" }
    - { entity: sensor.power_out, label: "Out", color: "#ff8800" }
  pos: { col: 0, row: 5, w: 6, h: 4 }
```

Legacy `type: sparkline` YAML still loads (single-entity Line mode).

### text

Static markdown text card.

```yaml
- type: text
  markdown: "## Notes\nMQTT down? check broker."
  pos: { col: 9, row: 0, w: 3, h: 4 }
```

### entity_list

Static list of entities from one instance.

```yaml
- type: entity_list
  instance: home
  entities:
    - light.kitchen
    - switch.coffee
    - sensor.front_door
  title: "Quick View"
  pos: { col: 0, row: 0, w: 4, h: 6 }
```

### filtered_entity_list

Dynamic entity list using a glob + filter query, updated live.

```yaml
- type: filtered_entity_list
  instance: home
  query: "light.*[domain=light][state=on]"
  hide_when_empty: true           # optional — hide whole card when no matches
  title: "Lights On"
  pos: { col: 4, row: 0, w: 4, h: 6 }
```

Query syntax: `glob[filter][filter]...` — e.g. `sensor.*_temp[state>20]`. With `hide_when_empty: true` the card auto-reappears within ≤250ms when an entity starts matching.

### clock

Local or timezone-aware clock with strftime format string.

```yaml
- type: clock
  format: "%H:%M:%S"             # strftime format
  timezone: "America/New_York"   # optional IANA timezone
  pos: { col: 8, row: 0, w: 4, h: 2 }
```

### statistics

Aggregated statistic over a rolling window — average, min, max, sum, or count.

```yaml
- type: statistics
  instance: home
  entity: sensor.power
  metric: avg                     # avg | min | max | sum | count
  window: "1h"                    # rolling window
  unit: "W"                       # optional unit label
  title: "Avg Power"
  pos: { col: 0, row: 8, w: 3, h: 2 }
```

### media_player

Now-playing card: title, artist, app name, and a volume gauge.

```yaml
- type: media_player
  instance: home
  entity: media_player.living_room
  pos: { col: 0, row: 10, w: 6, h: 3 }
```

When the card is selected on a dashboard:

| Key   | Service                  |
|-------|--------------------------|
| Space | `media_play_pause`       |
| `n`   | `media_next_track`       |
| `p`   | `media_previous_track`   |
| `+`   | `volume_up`              |
| `-`   | `volume_down`            |
| `m`   | `volume_mute` (toggle)   |

### image

Image entity (`image.*`) or camera proxy (`camera.*`) with optional auto-refresh.

```yaml
# Camera with 10s refresh
- type: image
  instance: home
  source:
    kind: camera
    entity: camera.front_door
  refresh_seconds: 10             # camera-only; omit for one-shot
  pos: { col: 6, row: 0, w: 6, h: 6 }

# Image entity — refetched whenever HA publishes a new frame
- type: image
  instance: home
  source:
    kind: image_entity
    entity: image.weather_radar
  pos: { col: 6, row: 6, w: 6, h: 6 }
```

Rendered via `ratatui-image`. The picker auto-detects the terminal's image protocol (Kitty / iTerm2 / Sixel / halfblocks). On iTerm2 the iTerm2 protocol is forced because the terminal partially advertises Kitty but only renders the iTerm2 protocol correctly. Override with `HA_TUI_IMAGE_PROTO=halfblocks|sixel|kitty|iterm2`.

### weather

Current conditions and optional forecast strip. Fetches forecasts via `weather.get_forecasts` (HA 2024+).

```yaml
- type: weather
  instance: home
  entity: weather.home
  show_forecast: true             # optional; default true
  forecast_days: 3               # 1–7; default 3
  title: "Home Weather"          # optional
  pos: { col: 0, row: 12, w: 6, h: 5 }
```

The card adapts its layout based on size:
- **Small** (h ≤ 2): single line — glyph + temperature + condition.
- **Normal**: header + humidity/wind attributes + forecast strip.
- **Large** (h ≥ 6): centered large glyph + temperature.

Forecasts refresh automatically every 30 minutes.

## Terminal requirements

- Linux / macOS: any modern terminal (iTerm2, Alacritty, kitty, WezTerm, gnome-terminal) works.
- Windows: use **Windows Terminal** or Alacritty. The legacy `conhost` terminal does not support SGR mouse encoding.
- 256-color or true-color recommended for the instance color palette.
- Some weather glyphs (☀ ⛅ ☁ 🌧 ❄ 🌫 ⛈) require a font with emoji/Unicode coverage (e.g. Nerd Fonts, Iosevka, or any system font on macOS/Linux).

## Architecture

- `src/main.rs` — entry; tokio runtime; ratatui init/restore.
- `src/app.rs` — `App` state, async event loop (`tokio::select!` over crossterm `EventStream`, mpsc `AppEvent` channel, tick).
- `src/ha/{client,protocol,registry}.rs` — one tokio task per instance, custom thin Home Assistant WebSocket protocol layer over `tokio-tungstenite`.
- `src/dashboard/{mod,layout,persist,editor}.rs` — dashboard model, grid → Rect math, YAML round-trip, editor reducer with undo stack.
- `src/screens/` + `src/ui/widgets/` — per-screen draw logic and reusable widgets.
- `src/ui/theme.rs` — instance color palette resolver.
- `src/actions.rs` — domain-aware default action for `Enter`.

## Status

This is an early build (v0.1). Implemented milestones:

- [x] Connect + auth to one or more HA instances
- [x] Live entity browser with color-coded multi-instance view
- [x] Service calls via Enter (domain defaults)
- [x] YAML dashboards with 12 card types
- [x] Interactive editor (mouse + keyboard, undo, save)
- [x] History backfill (sparkline + statistics cards)
- [x] Weather forecasts via `weather.get_forecasts`
- [x] Image / camera proxy cards
- [ ] Free-form service-call dialog
- [ ] OAuth (refresh-token) auth flow
- [ ] Windows CI build
- [ ] Comment-preserving YAML save

## License

MIT
