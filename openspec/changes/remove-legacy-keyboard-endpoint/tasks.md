## 1. Inventory and Lock the Routing Contract

- [x] 1.1 Record a symbol-level handoff at `openspec/handoffs/remove-legacy-keyboard-endpoint.md` that maps every production raw-key producer/consumer to its precedence, mutation, shell effect, presentation push, and target TuiRealm owner. Enumerate exhaustively, not by family name:
  - every `GlobalViewKey` and raw `*Key` `ShellRequest` variant (`ConfirmKey`, `DaemonLostKey`, `RemoteReanchorKey`, `ContextMenuKey`, `FeedsManageKey`, `PlaybackPromptKey`, `SavePlaylistKey`, `QueueKey`) **and** the two cursor-carrying raw-key variants `ServiceRequest::SettingsKey { cursor, key }` and `PersistRequest::SettingsKey { cursor, key }`;
  - every `TerminalObserverEvent::Key` producer and `to_crossterm_key_event` call site (16 components: audiobookshelf_book, audiobookshelf_podcast, browser, confirm, context_menu, daemon_lost, feeds_manage, home, music_workspace, playback_prompt, queue, remote_reanchor, root, save_playlist, settings, tv_workspace);
  - the two **direct** `handle_key_with_home_context` call sites in `shell_home.rs` (the Home context-menu `.` path under Queue focus) that bypass `handle_legacy_key` entirely;
  - the F1 Help-open special case in `handle_legacy_key` (handled at the Model level, outside `CONTEXT_STACK`, with a blocking-overlay guard);
  - the five blanket `push_*_content` re-projection calls in `handle_legacy_key` (Home, Emby browser, ABS podcast, ABS book, Music workspace) and which of the ~10 global handlers + queue + browse dispatch actually mutate each;
  - the pure-swallow surfaces (`handle_key_home`, `handle_key_feeds`) that are already component-owned and need no conversion, only a documented "no raw-key producer" note.
  Verify repository searches have no unexplained production match.

- [ ] 1.2 Extend the existing shell/TuiRealm integration tests with one table-driven production-style routing matrix covering blocking-overlay swallow, leaf fallthrough to exactly one subscription, no local/global double action, Queue-versus-Library focus, and playback gating/double-tap behavior; verify the focused integration tests pass before routing code changes.

- [x] 1.3 Record the load-bearing precedence quirks that must survive the conversion, each with a routing-matrix row: the `clear_queue_prompt_c` vs context-menu mutual exclusion (#135), the `Ctrl+a` enqueue-before-playback claim (#209), the `[`/`]` bracket key meaning different things under Queue vs Library focus, the `handle_lib_key` Ctrl/Alt catch-all swallow, the Space/Escape double-tap that *falls through on the first press*, and the Ctrl+/ terminal-encoding ambiguity (`Char('/')` vs `Char('_')` with CONTROL).

## 2. Activate Parent and Global Routing

- [ ] 2.1 Replace `key_policy.rs`'s descriptive/custom gates with concrete TuiRealm keyboard subscriptions for existing owners, including blocking-overlay exclusion, and verify the routing matrix proves one eligible owner per representative chord. Correct the lossy shadow-table gates while doing so: `queue_column_width` is gated on `PanelMode::Both` + Shift+Left/Right (not `IsMounted(Queue)`), and `playback` is a per-key command table (not a boolean).

- [ ] 2.2 Move application-wide overlay, force-clear, refresh, Panel-mode, tab-selection, and quit interpretation into `UiRoot` semantic requests; remove its `TerminalObserverEvent::Key` fallback and conversion-adapter use, then verify root/help/focus-restoration and routing-matrix tests pass. Include the F1 Help-open special case (currently in `handle_legacy_key`, with its blocking-overlay guard) and the destination-independent Alt-key path (`handle_key_alt`: Alt+Left/Right panel-focus switch, Alt+Up/Down tab cycle, catch-all swallow of other Alt chords).

- [ ] 2.3 Move playback and visualizer chord interpretation, eligibility, and Space/Escape double-tap state to the `Playback` Interactive Component with typed `PlaybackRequest`s; verify existing playback input tests and the playback rows of the routing matrix pass. This is not a static gate: preserve the per-key `resolve_key(InputContext::Playback, snapshot, chord)` resolution, the separate `idle_feed_command_for_key` path, and the 300ms double-tap timing (`last_space_press`/`last_esc_press`) that returns `None` (falls through) on the first press. Space/Escape are owned by one global handler per Decision 6: the global handler implements the double-tap, dispatches the existing typed first-press leaf request (`BrowserBack`/`TvBack`/`AudiobookshelfBookIntent::Play`/`PodcastEpisodeIntent::FocusOrPlay`) by focused leaf on the first press, and the playback command on the second press; leaves stop claiming Space/Escape. Delete the dead `ATTR_SKIP_INTRO_PROMPT_VISIBLE`/`ATTR_NEXT_UP_PROMPT_VISIBLE` attributes (the skip-intro/next-up prompts are already a focused modal; focus is their blocking mechanism, no attribute mirror needed).

- [ ] 2.5 Resolve the shared-globals crux (`q`, Tab/BackTab, `1`–`9`, `.`) currently claimed by `handle_global_view_key` ahead of panel dispatch. The `.` context-menu key is selection-dependent and must move to the focused destination, which emits the explicit target — following the pattern already established in `browser.rs` (`BrowserContextMenu { item }`) and `music_workspace.rs` (`MusicTrackContextMenu`), where the focused leaf resolves its own selected item locally and emits a typed request. The Home Continue Watching target (`home_cw_selected`/`cw_item`) is resolved by the Home component from Model-owned `home_content` (the same resolution site as today, but emitted by the component rather than threaded through every `CONTEXT_STACK` handler signature).

## 3. Replace Raw-Key Overlay and Form Requests

- [ ] 3.1 Convert Confirm, daemon-lost, remote-reanchor, context-menu, and playback-prompt components from raw-key shell requests to accept/cancel/move/submit/dismiss intents while reusing existing shell effect methods; verify the focused component and shell tests for those surfaces pass and the handoff matrix shows no raw-key producer for the family.

- [ ] 3.2 Convert Settings, Feeds management, Save-playlist, and remaining form/dialog raw-key requests to semantic intents with local text/navigation state retained in their components — including the cursor-carrying `ServiceRequest::SettingsKey { cursor, key }` and `PersistRequest::SettingsKey { cursor, key }` variants, not just the bare `*Key` forwards; verify focused Settings/Feeds/playlist tests pass and the handoff matrix shows no raw-key producer for the family.

## 4. Complete Queue Keyboard Ownership

- [ ] 4.1 Move Queue-local navigation, scope, width, clear, edit, save, and playback-action interpretation out of `App::handle_queue_key`/`QueueKey` into `QueueComponent`, emitting typed requests only for canonical Queue or shell effects and preserving targeted content pushes; verify focused Queue component/shell tests and Queue rows of the routing matrix pass. Preserve the `[`/`]` Local/Remote scope switch (which means something different under Library focus), the Shift+Up/Down reorder, the Ctrl+t/Ctrl+r remote-tracking keys, and the `QUEUE_NAV_CURSOR_HOLD` 500ms cursor-hold side effect.

## 5. Remove Library-Destination Fallback

- [ ] 5.1 Replace Home and generic Emby Browser `GlobalViewKey` fallthrough with component-local interpretation plus explicit selected-target requests, and verify focused Home/Browser component and shell tests pass with no raw-key producer remaining in those modules. Preserve the `handle_key_emby_library` interception branches (season/music-group/feed-group/pill switching, album-folder activation, series-selection) that run *before* `handle_lib_key`, and the `handle_lib_key` Ctrl/Alt catch-all swallow.

- [ ] 5.2 Replace TV and Music workspace `GlobalViewKey` fallthrough with component-local interpretation plus existing typed movement/effect requests, and verify focused TV/Music component and shell tests pass with no raw-key producer remaining in those modules.

- [ ] 5.3 Replace Audiobookshelf podcast and book `GlobalViewKey` fallthrough with component-local interpretation plus existing typed movement/effect requests, and verify focused Audiobookshelf component and shell tests pass with no raw-key producer remaining in those modules.

## 6. Delete the Legacy Endpoint and Ratchet the Boundary

- [ ] 6.1 After the handoff matrix reaches zero production raw-key consumers, delete `GlobalViewKey`, all remaining raw `*Key` request variants (including `ServiceRequest::SettingsKey`/`PersistRequest::SettingsKey`), `typed_key.rs`, `Model::handle_legacy_key`, `App::handle_key_with_home_context`, `CONTEXT_STACK`, obsolete context handlers/tests, and static-only policy scaffolding; replace the five blanket `push_*_content` re-projection calls in `handle_legacy_key` with targeted pushes at each request's handler (tracked per family in sections 2–5) before deleting the seam; verify `rtk cargo check -p mbv` succeeds and repository searches find no production legacy keyboard endpoint.

- [ ] 6.2 Extend the existing Interactive Component architecture gate to reject Crossterm `KeyEvent` payloads and raw fallback variants under `src/app/components/`, remove superseded legacy-loop characterization tests, and update the interactive-surface ledger verification record; verify `rtk ast-grep scan` and the focused architecture/input tests pass.

## 7. Final Verification

- [ ] 7.1 Run `rtk cargo fmt`, `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`, and `rtk cargo clippy --workspace --all-targets`; verify no new failure or warning remains.

- [ ] 7.2 Run `rtk ast-grep scan` and `rtk make check-code-file-lines`, then verify production searches have zero `CONTEXT_STACK`, `handle_legacy_key`, `handle_key_with_home_context`, `GlobalViewKey`, raw shell `*Key` request, and `to_crossterm_key_event` matches.
