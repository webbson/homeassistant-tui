# Configuration

ha-tui reads two YAML files from `~/.config/ha-tui/`:

| File | Purpose |
|---|---|
| `config.yaml` | HA instances, tokens, log level |
| `dashboards.yaml` | Dashboard definitions (cards, layout) |

On first run both files are auto-created if absent — `config.yaml` from the bundled example and `dashboards.yaml` as a one-card Welcome dashboard.

To pre-create them manually:

```bash
mkdir -p ~/.config/ha-tui
cp config/config.example.yaml      ~/.config/ha-tui/config.yaml
cp config/dashboards.example.yaml  ~/.config/ha-tui/dashboards.yaml
```

## config.yaml

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

### Fields

| Field | Description |
|---|---|
| `alias` | Short name shown in the UI and used in dashboard YAML |
| `url` | WebSocket URL of the HA instance (`wss://` for TLS) |
| `token` | Long-lived access token — can use `${ENV_VAR}` expansion |
| `token_file` | Path to a file containing the token (read at startup) |
| `color` | Optional named color or `#rrggbb`; defaults to auto-assigned from 8-color palette |
| `log_level` | `error` / `warn` / `info` / `debug` / `trace` (default `info`) |

Use exactly one of `token` or `token_file` per instance.

### Generating a token

In Home Assistant: **Profile → Long-Lived Access Tokens → Create Token**.

## File locations

| Platform | Config dir | Log dir |
|---|---|---|
| macOS | `~/.config/ha-tui/` | `~/Library/Application Support/ha-tui/log/` |
| Linux | `~/.config/ha-tui/` | `~/.local/share/ha-tui/log/` |

Note: ha-tui intentionally uses `~/.config` on macOS (not `~/Library/Preferences`) so paths are identical across platforms.

Locations can be overridden at launch:

```bash
ha-tui --config /path/to/config.yaml --dashboards /path/to/dashboards.yaml
```
