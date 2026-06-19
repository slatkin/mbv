# player.rs internals

`play()` and `play_playlist()` are thin setup functions (~50 lines each). Real logic lives in session structs owning all loop state:

- **`SingleSession`** — single-file playback. Owns `quit_at`, `stop_reported`, `pending_load` (bool), intro state, next-up threshold. Key methods: `handle_command`, `on_time_pos`, `on_playback_restart`, `on_end_file`, `on_shutdown`, `run`.
- **`PlaylistSession`** — multi-file mpv playlist. Same shape, plus `items: Vec<MediaItem>`, `current_idx`, `forced_idx`, `pending_load: u8` (counts in-flight EndFile events from ReplacePlaylist / initial jump), `stop_reported`.
- **`SessionReporter`** — cloneable, shared between the event-loop thread and the progress reporter thread. Holds `ids: Arc<Mutex<(item_id, msid, sid)>>` (one lock for all three so transitions are never torn), `is_audio: Arc<AtomicBool>`, `status: Arc<Mutex<PlayerStatus>>`. Key methods: `report_progress`, `report_stopped` (zeroes position for audio), `report_ping`, `start_item`, `transition_to`.
- **`ProgressGuard`** — owns the background progress-reporter thread; `stop_and_join()` signals it and waits.
- **`MpvSessionConfig`** — plain struct carrying headless/script/intro flags into the session.

**Critical invariant — `pending_load` in `PlaylistSession`:** always assign with `=`, never `+=`, in `ReplacePlaylist`. The count must exactly equal the EndFiles mpv will emit for that operation (1, or 2 if `start_idx > 0`). When `pending_load` drains to 0 in `on_end_file`, `stop_reported` is reset to `false` for the new item.

**Critical invariant — `SessionReporter.ids`:** all three of `item_id`, `msid`, `sid` are updated atomically inside a single `Mutex::lock` in `start_item`. Never add separate per-field locks — the progress thread reads all three in one lock acquisition to avoid sending torn (new item_id, old sid) reports to Emby.

`effective_playback_state()` returns `(active, active_idx, pos_ticks, runtime_ticks, is_paused)` — for remote sessions it extrapolates position forward from the last-polled value, but only when `!remote.is_paused`.
