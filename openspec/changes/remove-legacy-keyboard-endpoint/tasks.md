## 1. Inventory and Lock the Routing Contract

- [x] 1.1 Record a symbol-level handoff at `openspec/handoffs/remove-legacy-keyboard-endpoint.md` that maps every production raw-key producer/consumer to its precedence, mutation, shell effect, presentation push, and target owner. Enumerate exhaustively, not by family name:
  - every `GlobalViewKey` and raw `*Key` `ShellRequest` variant (`ConfirmKey`, `DaemonLostKey`, `RemoteReanchorKey`, `ContextMenuKey`, `FeedsManageKey`, `PlaybackPromptKey`, `SavePlaylistKey`, `QueueKey`) **and** the two cursor-carrying raw-key variants `ServiceRequest::SettingsKey { cursor, key }` and `PersistRequest::SettingsKey { cursor, key }`;
  - every `TerminalObserverEvent::Key` producer and `to_crossterm_key_event` call site (16 components: audiobookshelf_book, audiobookshelf_podcast, browser, confirm, context_menu, daemon_lost, feeds_manage, home, music_workspace, playback_prompt, queue, remote_reanchor, root, save_playlist, settings, tv_workspace);
  - the two **direct** `handle_key_with_home_context` call sites in `shell_home.rs` (`#[cfg(test)]`-only, not production bypasses);
  - the F1 Help-open special case in `handle_legacy_key` (Model level, outside `CONTEXT_STACK`, with a blocking-overlay guard);
  - the five blanket `push_*_content` re-projection calls in `handle_legacy_key` (Home, Emby browser, ABS podcast, ABS book, Music workspace) and which of the ~10 global handlers + queue + browse dispatch actually mutate each;
  - the pure-swallow surfaces (`handle_key_home`, `handle_key_feeds`) that are already component-owned and need no conversion, only a documented "no raw-key producer" note.
  Verify repository searches have no unexplained production match.

- [x] 1.3 Record the load-bearing precedence quirks that must survive, each with a routing-matrix row: the `clear_queue_prompt_c` vs context-menu mutual exclusion (#135), the `Ctrl+a` enqueue-before-playback claim (#209), the `[`/`]` bracket key meaning different things under Queue vs Library focus, the `handle_lib_key` Ctrl/Alt catch-all swallow, the Space/Escape double-tap that *falls through on the first press*, and the Ctrl+/ terminal-encoding ambiguity (`Char('/')` vs `Char('_')` with CONTROL).

- [x] 1.4 Amend the 1.1 handoff for ADR 0023 and the prompt removal: mark `playback_prompt` and `PlaybackPromptKey` as **deleted, not converted**; record the fourth input path (`notif_action_tx` → `drain_notif_actions`) and which of its arms are removed versus retained (`clear:yes`, `__notif_failed__` stay); record that `App.next_up_item` is retained as player state read by `PlayerEvent::NextUpPlay`, and that `App.skip_intro_end_ticks` becomes readerless. Verify the amended matrix accounts for every row in 1.1.

## 2. Land the Router Seam and the Integration Harness

- [x] 2.1 Add the `UiRoot` Keyboard Router seam per design Decision 1: a resolution function returning `Command` / `Swallow` / `FallThrough`, and a message fold in `shell_run.rs` that applies the outcome to the tick's leaf message (`Command`/`Swallow` discard it, `FallThrough` keeps it). Land it with **empty policy** — every chord resolves `FallThrough`, so `handle_legacy_key` still runs and behavior is unchanged. Replace `route_terminal_observer_message`'s focus check with the fold. Verify `rtk cargo nextest run -p mbv` is green and no observable behavior changed.

- [x] 2.2 Add the table-driven production-style routing matrix at `Application::tick()` level covering: blocking-overlay `Swallow` of an unbound and a global chord; a router `Command` discarding the focused leaf's message; a router `FallThrough` leaving exactly one leaf message standing with no global effect; Queue-versus-Library focus routing; playback gating; and the double-tap first-press fall-through / second-press claim. Include a row for each 1.3 quirk. Verify the matrix passes against current (empty-policy) behavior so it is trustworthy before policy moves.

## 3. Remove the Skip-Intro and Next-Up TUI Prompts

- [x] 3.1 Delete the prompt surface per design Decision 4: `PlaybackPromptComponent`, `ComponentId::PlaybackPrompt`, `ShellRequest::PlaybackPromptKey`, `sync_playback_prompt`, `render_playback_prompt`, `render_playback_prompt_content`, `handle_key_confirm_skip_intro`, `handle_key_confirm_next_up`, both `CONTEXT_STACK` and `KEY_POLICY` entries, the dead `ATTR_SKIP_INTRO_PROMPT_VISIBLE`/`ATTR_NEXT_UP_PROMPT_VISIBLE` attributes, both `self.status = "... (Y/n)"` writes with their `status_expires = None`, the two `notify_with_actions` calls in `player_event.rs`, and the `skip_intro:skip`/`next_up:play`/`next_up:skip` arms of `drain_notif_actions`. Keep the `clear:yes` and `__notif_failed__` arms.

- [ ] 3.2 Delete `App.skip_intro_end_ticks` and every write/clear site (`construct.rs`, `daemon_restart.rs`, `player_event.rs`, `session_command_actions.rs`, `session_connect.rs`, `session_switch.rs`, `emby_service_actions.rs`, `tests.rs`). **Retain `App.next_up_item` and all of its clear sites** — `PlayerEvent::NextUpPlay` reads it to resolve the `JumpTo` index. Verify `PlayerEvent::IntroStarted` still auto-seeks under `always_skip_intro`, and that `SkipIntroDismiss`/`NextUpDismiss`/`NextUpShow` still reach mpv unchanged.

- [ ] 3.3 Land the `toast-notification-semantics` spec delta narrowing both prompt carve-outs to the clear-queue confirmation, and add `docs/architecture/mpv-owned-playback-prompts.md`. Verify `openspec validate remove-legacy-keyboard-endpoint --strict` passes.

## 4. Activate the Policy and Move the Globals In

- [ ] 4.1 Turn `key_policy.rs` into the router's live ordered policy reading a plain-data snapshot; remove `#![allow(dead_code)]`, the `Custom("...")` gates, the `KeyPolicyGate::sub_clause()` bridge, and the tests comparing the table to `CONTEXT_STACK`. Correct the lossy gates recorded in design Decision 2: `queue_column_width` is `PanelMode::Both` + Shift+Left/Right (not `IsMounted(Queue)`); `playback` is the per-key `resolve_key` table plus the separate `idle_feed_command_for_key` path, not a boolean. Add no `SubClause` keyboard gates. Verify the routing matrix still passes.

- [ ] 4.2 Move the destination-independent globals into the router as `Command`/`Swallow`: `q`, Tab/BackTab, `1`–`9`, Ctrl+L force-clear, F5 refresh, Panel-mode cycle, overlay-open keys, the F1 Help-open case with its blocking-overlay guard, and the `handle_key_alt` path (Alt+Left/Right panel focus, Alt+Up/Down tab cycle, catch-all Alt swallow). Verify root/help/focus-restoration tests and the global rows of the matrix pass.

- [ ] 4.3 Move playback and visualizer resolution into the policy, preserving per-key eligibility, the `idle_feed_command_for_key` path, and the 300 ms double-tap (`last_space_press`/`last_esc_press`). Per design Decision 3, the first press resolves `FallThrough` so the focused leaf's existing request stands (`BrowserBack`/`TvBack`, `AudiobookshelfBookIntent::Play`, `PodcastEpisodeIntent::FocusOrPlay`) and the second within 300 ms resolves `Command`. Leaves keep their ordinary Space/Escape meanings and gain no timer. Verify the double-tap matrix rows pass.

- [ ] 4.4 Confirm blocking overlays resolve `Swallow` for every unmatched chord through the router rather than through per-component catch-alls, and verify the overlay-swallow matrix rows pass.

## 5. Replace Raw-Key Overlay and Form Requests

- [ ] 5.1 Convert Confirm, daemon-lost, remote-reanchor, and context-menu components from raw-key shell requests to accept/cancel/move/submit/dismiss intents, reusing existing shell effect methods. Verify the focused component and shell tests pass and the handoff matrix shows no raw-key producer for the family.

- [ ] 5.2 Convert Settings, Feeds management, and Save-playlist to semantic intents with local text/navigation state retained in their components — including `ServiceRequest::SettingsKey { cursor, key }` and `PersistRequest::SettingsKey { cursor, key }`, not just the bare `*Key` forwards. Verify focused Settings/Feeds/playlist tests pass and the handoff matrix shows no raw-key producer for the family.

## 6. Complete Queue Keyboard Ownership

- [ ] 6.1 Move Queue-local navigation, scope, width, clear, edit, save, and playback-action interpretation out of `App::handle_queue_key`/`QueueKey` into `QueueComponent`, emitting typed requests only for canonical Queue or shell effects and preserving targeted content pushes. Preserve the `[`/`]` Local/Remote scope switch (which means something different under Library focus), Shift+Up/Down reorder, Ctrl+t/Ctrl+r remote-tracking keys, and the `QUEUE_NAV_CURSOR_HOLD` 500 ms cursor-hold side effect. Verify focused Queue tests and the Queue rows of the matrix pass.

## 7. Remove Library-Destination Fallback

- [ ] 7.1 Replace Home and generic Emby Browser `GlobalViewKey` fall-through with component-local interpretation plus explicit selected-target requests. Per design Decision 6, `.` is a leaf request: the focused destination resolves its own selection and emits the target, and `HomeComponent` resolves the Continue Watching target from Model-owned `home_content`. Preserve the `handle_key_emby_library` interception branches (season/music-group/feed-group/pill switching, album-folder activation, series selection) that run *before* `handle_lib_key`, and the `handle_lib_key` Ctrl/Alt catch-all swallow. Verify focused Home/Browser tests pass with no raw-key producer in those modules.

- [ ] 7.2 Replace TV and Music workspace `GlobalViewKey` fall-through with component-local interpretation plus existing typed movement/effect requests. Verify focused TV/Music tests pass with no raw-key producer in those modules.

- [ ] 7.3 Replace Audiobookshelf podcast and book `GlobalViewKey` fall-through with component-local interpretation plus existing typed movement/effect requests. Verify focused Audiobookshelf tests pass with no raw-key producer in those modules.

## 8. Delete the Legacy Endpoint and Ratchet the Boundary

- [ ] 8.1 After the handoff matrix reaches zero production raw-key consumers, delete `GlobalViewKey`, all remaining raw `*Key` request variants (including `ServiceRequest::SettingsKey`/`PersistRequest::SettingsKey`), `typed_key.rs`, `Model::handle_legacy_key`, `App::handle_key_with_home_context`, `CONTEXT_STACK`, and the obsolete context handlers and their tests. Replace the five blanket `push_*_content` re-projection calls with the targeted pushes landed per unit in sections 4–7 before deleting the seam. Verify `rtk cargo check -p mbv` succeeds and repository searches find no production legacy keyboard endpoint.

- [ ] 8.2 Extend the Interactive Component architecture gate to reject Crossterm `KeyEvent` payloads and raw fallback variants under `src/app/components/`, add a gate rejecting a second keyboard resolution site outside the router (ADR 0023's one-router rule), remove superseded legacy-loop characterization tests, and update the interactive-surface ledger verification record. Verify `rtk ast-grep scan` and the focused architecture/input tests pass.

## 9. Final Verification

- [ ] 9.1 Run `rtk cargo fmt`, `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`, and `rtk cargo clippy --workspace --all-targets`; verify no new failure or warning remains.

- [ ] 9.2 Run `rtk ast-grep scan` and `rtk make check-code-file-lines`, then verify production searches have zero `CONTEXT_STACK`, `handle_legacy_key`, `handle_key_with_home_context`, `GlobalViewKey`, raw shell `*Key` request, `to_crossterm_key_event`, `PlaybackPromptComponent`, and `skip_intro_end_ticks` matches.
