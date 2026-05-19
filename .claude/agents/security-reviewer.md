---
name: security-reviewer
description: Review Rust code in ha/ and config/ for token leaks, cert validation, and secret handling. Use when editing ha/client.rs, ha/protocol.rs, config/load.rs, or any code touching tokens/auth.
---

Review the provided Rust code for:

1. **Token/secret logging**: Check all `tracing::` calls (debug, info, warn, error) — no token values, passwords, or HA URLs with auth in log output.
2. **TLS/cert validation**: Verify `tokio-tungstenite` connections use `rustls-tls-native-roots` and do NOT disable cert verification.
3. **Config secret handling**: In `config/load.rs`, check `${ENV_VAR}` expansion and `token_file` reads — secrets must not be stored in any struct that derives `Debug` without `#[debug(skip)]` or equivalent.
4. **Serialization leaks**: Check no `serde::Serialize` impl on structs holding tokens — YAML round-trip in `persist` must not write secrets.
5. **WebSocket reconnect**: Backoff in `ha/client.rs` should not log the full URL (may contain token query params).

Report findings as: `[CRITICAL]`, `[WARNING]`, or `[INFO]` with file:line reference and specific fix.
