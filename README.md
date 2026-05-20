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

Copy the example files into your XDG config dir — `~/.config/ha-tui/` on both Linux and macOS:

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
| `q` / `Esc` | Quit |
| `E` | Entity browser |
| `i` | Instances screen |
| `1`..`9` | Jump to dashboard N |
| `e` | Edit the currently shown dashboard |
| `?` | Help overlay |

### Entity browser

| Key | Action |
|---|---|
| `j`/`k` or `↓`/`↑` | Navigate |
| `PageUp`/`PageDown` | Jump 10 |
| `Home`/`End` | First / last |
| `f` | Cycle instance filter (all → first → next → …) |
| `Enter` | Domain-default action (toggle lights/switches, turn_on scripts/scenes, etc.) |

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

HA-style horizontal arc with severity zones and optional needle.

```yaml
- type: gauge
  instance: cabin
  entity: sensor.cabin_temp
  min: -20
  max: 30
  unit: "°C"
  needle: true                    # optional needle marker
  severity:                       # optional color zones
    - { from: -20, to: 0, color: blue }
    - { from: 0,  to: 20, color: green }
    - { from: 20, to: 30, color: red }
  pos: { col: 0, row: 2, w: 4, h: 3 }
```

### graph

Multi-series sparkline with history backfill from HA. Supports Line, Bar, and Pie chart styles.

```yaml
- type: graph
  instance: home
  entity: sensor.power
  window: "1h"                    # history window: 1h, 6h, 24h, 7d, etc.
  chart: line                     # line | bar | pie
  pos: { col: 0, row: 5, w: 6, h: 4 }
```

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
  title: "Lights On"
  pos: { col: 4, row: 0, w: 4, h: 6 }
```

Query syntax: `glob[filter][filter]...` — e.g. `sensor.*_temp[state>20]`.

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

### image

Image entity or camera proxy with optional auto-refresh.

```yaml
- type: image
  instance: home
  entity: camera.front_door
  refresh_interval: 10            # seconds; optional
  pos: { col: 6, row: 0, w: 6, h: 6 }
```

For `image` entities (static image): omit `refresh_interval` or set to `0`.

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
