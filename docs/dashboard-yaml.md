# Dashboard YAML reference

Dashboards are defined in `~/.config/ha-tui/dashboards.yaml`. A working example is at [`config/dashboards.example.yaml`](../config/dashboards.example.yaml).

Most editing can be done in-app (`e` to open the editor). This reference is for hand-editing or understanding the full schema.

## Layout

`pos` units are grid cells, not characters. `grid` defaults to `{ cols: 12, rows: 24 }`.

Two layout types are supported (`Free` is default when `layout:` is absent):

- **Free** (`layout:` absent or `type: free`) — cards have explicit `pos: { col, row, w, h }` on a named grid.
- **Grid** (`type: grid`) — structured rows/columns; cards stack at natural height.

### Grid layout YAML

```yaml
- name: Stacked
  layout:
    type: grid
    rows:
      - height: 4          # fixed terminal rows
        columns:
          - cards: [{type: entity, instance: home, entity: light.kitchen}]
          - cards: [{type: clock, format: "%H:%M:%S"}]
      - height: auto       # splits leftover space with other auto rows
        fill_height: true  # scale card heights to fill column (default for row's cols)
        columns:
          - cards: [...]
          - fill_height: false  # per-column override
            cards: [...]
```

## Common card fields

Every card supports:

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

## Card types

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
