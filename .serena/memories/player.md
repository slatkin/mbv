# player.rs

`Player` wraps libmpv2. Runs in its own thread. Receives `PlayerCommand` via mpsc, sends `PlayerEvent` back to App.

## App handles PlayerEvent to:
- Update played status
- Advance queue
- Report progress to Emby: `report_start`, `report_progress_ws`, `report_stopped`

## Session structs (critical invariants)

See `docs/player-internals.md` for `SingleSession`, `PlaylistSession`, `SessionReporter` details and invariants. **Read that doc before touching player.rs session state** (`pending_load`, `SessionReporter.ids`).

## Lang table sync

`lang_code_to_name()` here must match `parse_audio_info` lang table in `api.rs`. Same ISO 639-1/2 → English names. If you touch one, check the other.

## Daemon boundary

`player.rs` ↔ `daemon.rs` / `remote_player.rs` boundary is sensitive. Do not change without understanding the full daemon/remote_player flow.
