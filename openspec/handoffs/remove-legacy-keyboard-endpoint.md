# Symbol-level handoff — remove-legacy-keyboard-endpoint

Base commit: `23466ac8`. Change: `openspec/changes/remove-legacy-keyboard-endpoint/`.
This is the task 1.1 + 1.3 inventory. Every claim cites the current source at that base.

All `*Key` raw-key variants carry a `crossterm::event::KeyEvent`, reconstructed by
`to_crossterm_key_event` (`src/app/components/typed_key.rs:8`) — the TuiRealm→Crossterm
adapter the change deletes.

Consumers all live in `Model::handle_terminal_message` (`src/app/shell_messages.rs:8`),
except the `TerminalObserverEvent::Key` path which is `apply_terminal_observer`
(`src/app/shell.rs:98`).

---

## 1. Raw `*Key` ShellRequest variants: producer → consumer → effect → push → owner

### 1.1 `ShellRequest::GlobalViewKey(KeyEvent)` (msg.rs:233)

Producers (non-test):
| Component | site | when emitted |
|---|---|---|
| `home.rs` | 345 (Alt+arrows), 348 (!focused), 401 (unmatched `_`), 409 (Alt+arrows via `handle_key`) | Home forwards unclaimed/Alt/unfocused keys so `handle_queue_key`/globals retain authority |
| `browser.rs` | 307 (Alt+arrows), 500 (unmatched after local set), 530 (unmatched with no item) | generic Emby browser fall-through |
| `music_workspace.rs` | 238 (Enter, no track focus, narrow), 356 (unmatched `_`) | music workspace fall-through |
| `tv_workspace.rs` | 233 (!focused), 333 (unmatched `_`) | TV workspace fall-through |
| `audiobookshelf_book.rs` | 146 (!focused), 250 (unmatched `_`) | ABS book fall-through |
| `audiobookshelf_podcast.rs` | 264 (unmatched `_`) | ABS podcast fall-through |

Consumer: `shell_messages.rs:19-22` — `if self.handle_legacy_key(key) { quit = true; }`.
This routes into `Model::handle_legacy_key` (shell.rs:128), which runs the **F1 Help
special case** + `App::handle_key_with_home_context` (full `CONTEXT_STACK`) + the **five
blanket pushes** (shell.rs:150/152/156/159/160).

Effect set: whatever `CONTEXT_STACK` assigns — quit (`q`), tab cycle (`Tab`/`BackTab`),
tab jump (`1`–`9`), context menu (`.`), Alt panel/tab, queue keys, browse keys, playback.

Owner target: **focused Library destination** for `.`, **UiRoot** for q/Tab/1-9/Alt,
**Queue** for queue chords, **Playback** for playback chords (design §2.5, D2).

### 1.2 `ShellRequest::ConfirmKey(KeyEvent)` (msg.rs:249)

Producer: `confirm.rs:114` — every `Event::Keyboard` forwarded (modal swallows).
Also feeds remove-confirm path reuses Confirm.

Consumer: `shell_messages.rs:99-105` → `Model::handle_confirm_key` (`shell_modal_actions.rs:10`):
reads `ConfirmComponent::confirm_action`, dismisses modal if `confirm_key_dismisses`
(`shell_modal_actions.rs:167`), then `App::apply_confirm_action` (input_confirm_keys.rs:19).
Pushes (in consumer): `push_home_content` (:101), `push_emby_browser_content` (:103).

Owner target: **Confirm component accepts/cancels locally; shell executes effects** (design §3.1).

### 1.3 `ShellRequest::DaemonLostKey(KeyEvent)` (msg.rs:250)

Producer: `daemon_lost.rs:102` — every keyboard event.

Consumer: `shell_messages.rs:106-110` → `Model::handle_daemon_lost_key` (`shell_modal_actions.rs:26`):
`r`/`R` → `restart_local_daemon(true)`, `s`/`S` → `restart_local_daemon(false)`, `q`/`Q`
→ `dismiss_modal` + `try_quit`. No push in consumer.

Owner target: **DaemonLost component local restart intent; shell executes process lifecycle** (§3.1).

### 1.4 `ShellRequest::RemoteReanchorKey(KeyEvent)` (msg.rs:254)

Producer: `remote_reanchor.rs:97` — every keyboard event.

Consumer: `shell_messages.rs:111-115` → `Model::handle_remote_reanchor_key` (`shell_modal_actions.rs:61`):
`Esc` → dismiss, `Up`/`Down` → `RemoteReanchorComponent::move_cursor`, `Enter` →
`selected_target` + `App::reanchor_remote_target`. No push in consumer.

Owner target: **RemoteReanchor component local cursor + confirm; shell dismiss/reanchor** (§3.1).

### 1.5 `ShellRequest::ContextMenuKey(KeyEvent)` (msg.rs:258)

Producer: `context_menu.rs:192` — every keyboard event.

Consumer: `shell_messages.rs:117-123` → `Model::handle_context_menu_key` (`shell_overlays_menus.rs:159`):
`Up`/`Down` → `ContextMenuComponent::move_cursor`, `Enter` → `action_at(cursor)` +
`dismiss_context_menu` + `App::execute_context_action(action, home_cw_item())`, `Esc` → dismiss.
Pushes (in consumer): `push_home_content` (:119), `push_emby_browser_content` (:121).

Owner target: **ContextMenu component local cursor; shell executes menu actions** (§3.1).

### 1.6 `ShellRequest::FeedsManageKey(KeyEvent)` (msg.rs:318)

Producer: `feeds_manage.rs:130-132` — `shell_key()` called for `Enter`/`Esc` when editing
a form field (list/other edits are component-local).

Consumer: `shell_messages.rs:206-208` → `handle_feeds_manage_request` (`shell_overlays_menus.rs:739`)
→ `handle_feeds_manage_key` (`shell_feeds_manage.rs:78`): stage List → `Esc` dismiss,
`a` start-add, `Enter`/`e` start-edit, `d` confirm-remove; stage Form → `Esc` cancel,
`Enter` (not submitting) submit. No push in consumer.

Owner target: **FeedsManage component owns text/form state; shell owns submit/remove
effects** (§3.2).

### 1.7 `ShellRequest::PlaybackPromptKey(KeyEvent)` (msg.rs:320)

Producer: `playback_prompt.rs:79-81` — every keyboard event.

Consumer: `shell_messages.rs:485-491` — if `App.skip_intro_end_ticks.is_some()` →
`handle_key_confirm_skip_intro` (input_confirm_keys.rs:185); else if
`App.next_up_item.is_some()` → `handle_key_confirm_next_up` (input_confirm_keys.rs:210).
`y`/`Y`/`Enter` confirm; any other key dismisses. No push in consumer.

Owner target: **`PlaybackPromptComponent` is already a focused modal (`application.active()` in `sync_playback_prompt`); focus is the blocking mechanism.** The component interprets `y`/`Y`/`Enter` → Confirm, any other key → Dismiss locally (task 3.1), emitting a typed `PlaybackPromptIntent`; the shell consumer at `shell_messages.rs:485` keeps choosing `handle_key_confirm_skip_intro` vs `_next_up` by App field. No attribute mirror; `ATTR_SKIP_INTRO_PROMPT_VISIBLE`/`ATTR_NEXT_UP_PROMPT_VISIBLE` are dead (init `false`, never read as guards) and are deleted with task 2.3. Task 2.4 was removed from the plan.

### 1.8 `ShellRequest::SavePlaylistKey(KeyEvent)` (msg.rs:404)

Producer: `save_playlist.rs:72-74` — `handle_key` appends printable chars /
Backspace locally, then forwards EVERY key (including the consumed ones).

Consumer: `shell_messages.rs:471-473` → `Model::handle_save_playlist_key` (`shell_modal_actions.rs:95`):
`Esc` → dismiss + `force_clear`; `Enter` → rename (spawn) or save-as (existing → overwrite
Confirm, else `save_queue_as_playlist`). No push in consumer.

Owner target: **SavePlaylist component owns input; shell owns dismiss/save/rename** (§3.2).

### 1.9 `ShellRequest::QueueKey(KeyEvent)` (msg.rs:406)

Producer: `queue.rs:163` — `handle_key` unmatched-after-local-claims fall-through
(the only `QueueKey` production site; all other queue keys emit typed `QueueRequest`s).

Consumer: `shell_messages.rs:474-477` — `if self.app.handle_queue_key(key) { quit = true; }`.
`App::handle_queue_key` (input_queue_keys.rs:65) handles the rest of Queue routing:
`[`/`]` scope, cursor, page, `Ctrl+t`/`Ctrl+r` remote tracking, `Shift+Up/Down` reorder,
`Ctrl+z` undo, `i` navigate, `p` play-now, `Ctrl+s` save dialog. **Note: no `push_*` in
this consumer** — queue swaps scope/key effects that mutate `sync_queue`-driven content. No push here.

Owner target: **QueueComponent** (§4.1).

### 1.10 cursor-carrying `ServiceRequest::SettingsKey { cursor, key }` (msg.rs:101-104)

Producer: `settings.rs:144-147` — `service_key()` for the Services destination on
`Enter`/`Space`/`d`/`D`/`t`/`T`/`r`/`R`.

Consumer: `shell_settings.rs:137-160` (`handle_service_request`) — mounts Settings sidebar,
sets `settings_destination = Services`, `services_cursor = cursor`, then dispatches by key:
`Enter`/`Space` → `activate_service_entry`, `d`@0 → `request_emby_removal`, `t`@1 →
`test_audiobookshelf_connection`, `r`@1 → ReplaceAudiobookshelf, `d`@1 → RemoveAudiobookshelf.
No push in consumer.

Owner target: **Settings component local cursor; shell service effects** (§3.2).

### 1.11 cursor-carrying `PersistRequest::SettingsKey { cursor, key }` (msg.rs:121-123)

Producer: `settings.rs:254-257`, `:264-267`, `:286-289` (non-Services destinations and
Services Esc/F3/F4/q), `:321-325` (mouse-click synthesised Enter key).

Consumer: `shell_settings.rs:224-271` (`handle_persist_request`) — Services: `Esc` →
destination Main, `F3` → Sessions, `F4` → Playlists, `q` → `try_quit`; Main: `Esc` →
`close_settings`, `F3`/`F4` → switch sidebar, `q` → quit, `Left`/`Right`/`Space`/`Enter`
→ `settings_cursor = cursor` + `handle_settings_activate`. No push in consumer.

Owner target: **Settings component local cursor/nav; shell settings effects** (§3.2).

---

## 2. `TerminalObserverEvent::Key` producer + `to_crossterm_key_event` call sites (16 components)

`TerminalObserverEvent::Key` producer (the only one): **`root.rs:73`** — `UiRootComponent::on`
converts every `Event::Keyboard` via `to_crossterm_key_event` into
`Msg::TerminalEvent(TerminalObserverEvent::Key(_))`. UiRoot subscribes with
`EventClause::Any` + `SubClause::Always` (`root.rs:42`).

Routing: `shell_run.rs:388` → `route_terminal_observer_message` (shell.rs:86) **drops the
Key message when `focused != UiRoot`**. When `focused == UiRoot`, `apply_terminal_observer`
(shell.rs:107-110) → `Model::handle_legacy_key(key)`.

`to_crossterm_key_event` call sites (non-test) per component:
| Component | call sites |
|---|---|
| audiobookshelf_book | 147, 251 |
| audiobookshelf_podcast | 265 |
| browser | 531, 534 |
| confirm | 113 |
| context_menu | 191 |
| daemon_lost | 101 |
| feeds_manage | 131 |
| home | 410, 413 |
| music_workspace | 239, 357 |
| playback_prompt | 80 |
| queue | 163 |
| remote_reanchor | 96 |
| root | 73 |
| save_playlist | 73 |
| settings | 146, 256, 266, 288, 323 |
| tv_workspace | 234, 333 |

(40 production call sites total; 14 `use` imports. `typed_key.rs` itself is the adapter.)

---

## 3. The two DIRECT `handle_key_with_home_context` call sites in `shell_home.rs`

These bypass `handle_legacy_key` entirely (they call `App::handle_key_with_home_context`
directly — no F1 case, no blanket pushes; the caller re-projects separately):

1. `shell_home.rs:655-661` — in the `shell_home.rs` `#[cfg(test)] mod tests` module
   (tests begin at shell_home.rs:232):: `.` key under `PanelFocus::Queue` while Home is the
   active Tab. Passes `model.home_continue_watching_selected()` and `model.home_cw_item()`.
   Asserts the queue context menu contains the "Remove from Continue Watching" entry when
   `home_continue_watching_selected()` is true.
2. `shell_home.rs:701-707` — same path after restoring the component to Continue Watching
   (section 0): asserts the entry appears again.

`home_continue_watching_selected()` (shell_home_content.rs:211) resolves the fact from the
mounted `HomeComponent::section() == 0`. `home_cw_item()` (shell_home_content.rs:122)
returns `home_content.continue_items[continue_cursor]`. Both are Model-owned getters that
thread through `CONTEXT_STACK` → `handle_global_view_key` → `App::open_context_menu`
(context_menu_actions.rs:596).

**Test-only call sites** additionally exist at `input_movie_detail_tests.rs:304` and
`tests_context_menu_placement.rs:129` (same production front door). These are assertions,
not production routing.

---

## 4. F1 Help-open special case (Model level, outside CONTEXT_STACK)

`shell.rs:128-138` — `Model::handle_legacy_key`:

```rust
let quit = if key.code == crossterm::event::KeyCode::F(1)
    && !self.application.mounted(&ComponentId::Overlay(OverlayId::Help))
    && !self.is_blocking_overlay_open()   // shell.rs:129 → blocking_overlay_active() (shell_root.rs:47)
{
    self.mount_help();   // shell_overlays_sidebars.rs:84
    false
} else {
    self.app.handle_key_with_home_context(key, self.home_continue_watching_selected(), self.home_cw_item())
};
```

Blocking overlay set (shell_root.rs:47-64): ContextMenu, SelectionModal, Confirm,
DaemonLost, RemoteReanchor, SavePlaylist, Multiselect, LibraryRoutes, FeedManage.
Once Help is mounted it is the active component, so subsequent F1 arrives as
`Msg::Shell(DismissHelp)` (Help component). This special case must move into UiRoot's
semantic requests (task 2.2) with its blocking-overlay guard.

---

## 5. Five blanket `push_*_content` re-projection calls in `handle_legacy_key`

| # | push | shell.rs line | what re-projects |
|---|---|---|---|
| 1 | `push_home_content` | 150 | Home component content/focus |
| 2 | `push_emby_browser_content` | 152 | generic Emby Browser content |
| 3 | `push_audiobookshelf_podcast_content` | 156 | ABS podcast content |
| 4 | `push_audiobookshelf_book_content` | 159 | ABS book content |
| 5 | `push_music_workspace_content` | 160 | Music workspace content |

**Which handlers actually mutate which surface** (each family replaces only its own):

- **q (quit)**: `try_quit` (consume_quit_actions.rs:15) — no content mutation. No push needed.
- **Tab/BackTab/1-9 (tab switch)**: `library_tab_next/prev` (cw_library_tab_actions.rs:77/87),
  `set_library_tab` (:69) → `apply_tab_position` — **changes the active destination**, so the
  mounted surface's content (Home / one Emby library / one ABS lib / Feeds) is re-projected:
  `push_home_content` for Home, `push_emby_browser_content` for Emby library,
  `push_audiobookshelf_{podcast,book}_content` for ABS.
- **`.` (context menu)**: `open_context_menu` (context_menu_actions.rs:596) — sets
  `pending_overlay`, no browse surface mutation. But the menu's *execution*
  (`ContextMenuKey`/`ContextMenuSelect` consumers) re-pushes home+browser (:119/:121).
- **Alt+Left/Right (panel focus)**: `set_panel_focus` (panel_focus_state.rs:44) — changes
  `panel_focus`/`mini_view_focus`, which the **focused flag** of every projected component
  derives from (`push_*_content` readers use `effective_panel_focus`); all five pushes
  currently run.
- **Alt+Up/Down (tab cycle)**: `library_tab_prev/next` — as tab switch above.
- **Ctrl+l (force clear)**: sets `force_clear = true` — render flag only, no content. No push.
- **F5 (refresh)**: `refresh_current_view` (library_load_actions.rs:73) — mutates the active
  destination: Queue → `refresh_queue`; Home → `fetch_home` (+`LibEvent::HomeContentRefreshed`);
  Emby library → `refresh_lib`; ABS book → `audiobookshelf_book_refresh`, else
  `audiobookshelf_refresh`; Feeds → `refresh_feeds`. → needs the matching targeted push.
- **F2/F3/F4/Ctrl+/ (overlay open)**: mount/switch sidebars — no browse-surface mutation.
- **Ctrl+a (enqueue, handle_lib_key)**: `enqueue_selected` — mutates the **queue**,
  re-projected via `sync_queue`/push paths owned by Queue; browser content unchanged.
- **Queue keyboard family (`handle_queue_key`)**: cursor/scope/reorder/undo/save —
  mutates queue state, projected via queue sync; no Home/Emby/ABS/Music mutation.
- **`handle_key_emby_library` interception branches**: season/music-group/feed-group/pill
  switching and album-folder/series-selection — mutates **Emby browser or Music workspace
  content** (`push_emby_browser_content`/`push_music_workspace_content`).
- **`handle_lib_key` movement**: `move_lib_cursor{,_rows}`, `jump_lib_cursor`, etc. —
  mutates Emby browser content (`push_emby_browser_content`).
- **`handle_key_playback_key` / visualizer / confirm prompts**: mutate player/status state
  only — projected via the playback/prompt components, not the five pushes.
- **`handle_key_push_queue` scope change (`[`/`]`)**: `set_queue_scope` — queue only.

**Net for the deletion unit (6.1):** only tab switch, panel-focus, F5 refresh, and the
Emby-library interception run mutating pushes is accurate; the current five blanket calls
over-push. Each family lands its own targeted push (tracked per family, sections 2–5 of
tasks.md).

**Pure-swallow surfaces (no raw producer, no conversion needed):**
- `handle_key_home` (input_browse_dispatch.rs:79): `_key` → `Some(false)` — Home local
  keys are owned by `HomeComponent`; legacy handler swallows everything.
- `handle_key_feeds` (input_feed_tab_keys.rs:7): `_key` → `Some(false)` — Feeds local
  keys owned by `FeedsComponent`.
Record both with "no raw-key producer" documentation note in the handoff matrix.

---

## 6. Load-bearing precedence quirks (task 1.3) + routing-matrix rows

Each row: `chord | focus/context | expected owner | expected effect | matrix assertion`.

**(a) `clear_queue_prompt_c` vs context-menu mutual exclusion (#135)** — `handle_key_clear_queue_prompt`
(input_confirm_keys.rs:242): `char 'c'`, non-Alt, **unconditionally** opens-queue-confirm;
gated on *not* matching an open context menu (menu has no 'c' binding and must swallow it).
`CONTEXT_STACK` ordering puts `clear_queue_prompt_c` below the overlays; the queue
`handle_queue_key` 'c' must not double-fire.
Matrix row: `'c' (no Alt) | Queue focus, no overlay | Queue (clear-queue prompt) | open Clear-queue Confirm modal | exactly one pending overlay, no context menu, guard proves menu swallow`

**(b) `Ctrl+a` enqueue-before-playback claim (#209)** — `handle_enqueue_selected_key`
(input_lib_keys.rs:96-112): `Ctrl+a` under Library focus means enqueue-selected, claimed
BEFORE the Playback context's `'a'` (!ctrl → `ToggleMuteOrCycleAudio`,
action.rs:113-145) ever sees it. `playback_command_for_key`'s `'a'` is `!ctrl`-guarded.
Matrix row: `Ctrl+a | Library focus, player active | focused Browser destination (enqueue-selected) | enqueue_selected(lib_idx) | audio does NOT toggle, exactly one effect`

**(c) `[`/`]` bracket meaning split** — Queue focus: `handle_queue_key`
(input_queue_keys.rs:68-90) claims `[`/`]` (no Ctrl/Alt, `has_direct_remote_queue`) as
Local/Remote scope switch. Library focus: `handle_key_emby_library`
(input_browse_dispatch.rs:89-130) claims `[`/`]` as season/music-group/feed-group/pill
cycling; ABS components claim them as bucket/filter cycles.
Matrix row: `'[' | Queue focus | QueueComponent (scope→Local) | set_queue_scope(Local) | scope changes, library cursor untouched—or BrowserCycleLetterPill under Library focus`
Matrix row: `']' | Library focus (Emby lib) | focused destination | cycle_letter_pill(+1) / season switch | scope unchanged, pill/season advances`

**(d) `handle_lib_key` Ctrl/Alt catch-all swallow** — input_lib_keys.rs:207-211:
`KeyCode::Char(_) if CONTROL || ALT => {}` swallows unmapped Ctrl/Alt chords while a
library sub-panel is focused so they never leak to an unrelated queue shortcut.
Matrix row: `Ctrl+z | Library focus | focused destination (swallow) | no effect (no queue undo) | queue undo NOT fired, key consumed`

**(e) Space/Escape double-tap falling through on the first press** — `handle_playback_key`
(input_lib_keys.rs:326-359): `TogglePlayPause` (`Space`) and `Stop` (`Esc`) use 300ms
`last_space_press`/`last_esc_press`; **first press returns `None` (falls through) and
starts the window**; second press within 300ms dispatches. Must survive into the Playback
component (task 2.3) with identical timing.
Matrix row: `Space | playback active | Playback component | first: fall-through (no toggle), second ≤300ms: TogglePlayPause | exactly one dispatch after two presses, none after one`

**(f) Ctrl+/ terminal-encoding ambiguity** — `handle_key_global_overlay_open`
(input.rs:122-131): matches `Char('/')` OR `Char('_')` with CONTROL (different terminals /
kitty protocol) → `open_search_sidebar`.
Matrix row: `Ctrl+/ (Char('/')) AND Ctrl+/ (Char('_')) | no overlay | UiRoot (overlay-open) | open_search_sidebar | same effect for both encodings`

**Additional matrix rows from task 1.1 required coverage** (blocking-overlay swallow +
leaf fallthrough + no-local/global double action + Queue-vs-Library focus + playback
gating) form the U2 integration test harness; each quirk above is its own row there.
The existing `CONTEXT_STACK` order-pinning test (`input_resolver_handle_key_tests.rs`)
and `KEY_POLICY` mirror test (`key_policy.rs:224+`) must be repointed by the conversion.

---

## 7. Verification record (task 1.1 item 8)

Run at base `23466ac8`:

| search | matches (all files incl. tests/comments) | production consumers remaining? |
|---|---|---|
| `handle_legacy_key` | 5 | yes (shell.rs def, shell_messages.rs:19, comments) |
| `handle_key_with_home_context` | 10 | yes (shell.rs:141; direct test sites) |
| `CONTEXT_STACK` | 24 | yes (input_resolver.rs const) |
| `GlobalViewKey` | 42 | yes (7 prod emitters + consumer) |
| `to_crossterm_key_event` | 40 | yes |

No *unexplained* production matches: every hit traces to a producer/consumer above.

Explicitly **excluded** (not legacy-keyboard): `BroadcastKey`, `PlaybackKeyCode`,
`KeyModifiers`, media keys, `MediaKeyCode` hits in `typed_key.rs`, `Keyboard` event
payloads, and the `SyncKey`/`ui_util` naming noise.

---

## 8. Amendment for ADR 0023 + prompt removal (task 1.4)

Amends the 1.1 inventory for the routing decision (`docs/adr/0023-one-central-keyboard-router.md`)
and the skip-intro/next-up prompt removal (design Decision 4). This is the
**conversion checklist**: every row in sections 1–6 must be accounted for before
the final deletion unit (8.1) runs.

### 8.1 `playback_prompt` and `PlaybackPromptKey`: **deleted, not converted**

`ShellRequest::PlaybackPromptKey` (msg.rs:320) and its producer `playback_prompt.rs`
are **removed outright** — mpv's on-screen Skip Intro / Next Up buttons
(`scripts/mbv_intro.lua`, `scripts/mbv_visibility.lua`) become the sole interface.
No semantic replacement request exists or is added. This removes from the
conversion checklist:

- the producer rows for `playback_prompt` (handoff §1.7, `to_crossterm_key_event` table);
- the consumer `shell_messages.rs:485-491` (the `handle_key_confirm_skip_intro` /
  `handle_key_confirm_next_up` dispatch) and the handlers in `input_confirm_keys.rs`;
- both `CONTEXT_STACK` and `KEY_POLICY` entries (`confirm_skip_intro`,
  `confirm_next_up`), the dead `ATTR_SKIP_INTRO_PROMPT_VISIBLE` /
  `ATTR_NEXT_UP_PROMPT_VISIBLE` attributes, `ComponentId::PlaybackPrompt`,
  `sync_playback_prompt`, `render_playback_prompt`, `render_playback_prompt_content`,
  `shell_playback_prompt.rs`, and both `self.status = "... (Y/n)"` writes with
  `status_expires = None` (`player_event.rs` QueueNextUp, `run_loop_drains.rs` IntroStarted path).

### 8.2 The fourth input path: `notif_action_tx` → `drain_notif_actions`

`App::notif_action_rx` is a fourth input path outside the routing policy,
delivered by `shell_run.rs:157` / `run_loop_drains.rs:141`. Arms after removal:

| arm | disposition |
| --- | --- |
| `skip_intro:skip` (run_loop_drains.rs:146) | **removed** — seeks via `skip_intro_end_ticks` + sends `SkipIntroDismiss`; the prompt is gone, Lua owns the seek |
| `next_up:play` (run_loop_drains.rs:154) | **removed** — the `notify_with_actions` producer is deleted; `PlayerEvent::NextUpPlay` (player_event.rs:320) keeps the equivalent JumpTo logic for mpv's button |
| `next_up:skip` (run_loop_drains.rs:170) | **removed** — sends `NextUpDismiss`; the button's own dismiss path covers this |
| `clear:yes` (run_loop_drains.rs:175) | **retained** — `dismiss_confirm()` + `replace_queue_or_prompt(PendingQueueAction::ClearQueue)` |
| `__notif_failed__` (run_loop_drains.rs:179) | **retained** — sets `notif_failed` |
| `_` fallback (run_loop_drains.rs:182) | retained — dismissed/"ignore"/"cancel"/empty leave the TUI prompt untouched |

The `notify_with_actions` producers in `player_event.rs` (QueueNextUp and the
IntroStarted arm) are removed with the prompts; `notify_with_actions` itself and
the `clear:yes` / `__notif_failed__` producers stay.

### 8.3 `App.next_up_item`: retained player state

**Kept** (`app_struct.rs:172`). `PlayerEvent::NextUpPlay` (player_event.rs:320)
reads it to resolve the `JumpTo` index when the user clicks mpv's on-screen
button; `notif_action_rx` may also `take()` it (run_loop_drains.rs:155). All
existing clear sites stand: `construct.rs`, `daemon_restart.rs`, `session_switch.rs`
(×3), `session_connect.rs`, `session_command_actions.rs`, `player_event.rs`
(×4 incl. the NextUpPlay take), `emby_service_actions.rs`, `tests.rs`,
`input_resolver_handle_key_tests.rs`, `shell_overlays_tests.rs`.

### 8.4 `App.skip_intro_end_ticks`: becomes readerless, then deleted

**Deleted** (app_struct.rs:171). After the prompt removal the field has only
writes and clears, no reader: the `IntroStarted` writer (player_event.rs:381),
the clear sites (`player_event.rs` ×3, `run_loop_drains.rs:147` arm removed,
`daemon_restart.rs`, `session_switch.rs` ×3, `session_connect.rs`,
`session_command_actions.rs`, `emby_service_actions.rs`, `tests.rs`,
`input_resolver_handle_key_tests.rs`, `shell_overlays_tests.rs`), and the
`KEY_POLICY` `Custom("skip_intro_end_ticks.is_some()")` gate (key_policy.rs:120).
`PlayerEvent::IntroStarted` must still auto-seek under `always_skip_intro` and
`SkipIntroDismiss` / `NextUpDismiss` / `NextUpShow` must still reach mpv unchanged.

### 8.5 Matrix coverage check (1.1 rows → routing-matrix rows)

| 1.1/1.3 row | disposition | matrix row / test |
| --- | --- | --- |
| `GlobalViewKey` producers (home/browser/music/tv/abs_book/abs_podcast) | converted section 7 | destination rows (2.2) |
| `ConfirmKey` (confirm.rs:114) | converted 5.1 | overlay-swallow rows |
| `DaemonLostKey` (daemon_lost.rs:102) | converted 5.1 | overlay-swallow rows |
| `RemoteReanchorKey` (remote_reanchor.rs:97) | converted 5.1 | overlay-swallow rows |
| `ContextMenuKey` (context_menu.rs:192) | converted 5.1 | `'c'`/menu mutual exclusion row (6a) |
| `FeedsManageKey` (feeds_manage.rs:130-132) | converted 5.2 | form rows |
| `PlaybackPromptKey` (playback_prompt.rs:79-81) | **deleted** (8.1) | n/a — no matrix row |
| `SavePlaylistKey` (save_playlist.rs:72-74) | converted 5.2 | form rows |
| `QueueKey` (queue.rs:163) | converted 6.1 | Queue rows + `[`/`]` split (6c) |
| `ServiceRequest::SettingsKey` / `PersistRequest::SettingsKey` (settings.rs) | converted 5.2 | form rows |
| `TerminalObserverEvent::Key` producer (root.rs:73) + `to_crossterm_key_event` (16 components) | seam 2.1 replaces the focus check with the fold; all call sites deleted by 8.1 | every row |
| F1 Help-open special case (shell.rs:128-138) | moved to router 4.2 with blocking-overlay guard | Help/overlay rows |
| Five blanket `push_*_content` (shell.rs:150/152/156/159/160) | replaced by targeted pushes per unit | per-unit tests |
| `handle_key_home` / `handle_key_feeds` pure swallows | no raw producer — documented only | n/a |
| clear-queue vs context-menu exclusion (6a) | `'c'` policy gate | `'c'` row |
| `Ctrl+a` enqueue-before-playback (6b) | policy ordering | `Ctrl+a` row |
| `[`/`]` Queue-vs-Library split (6c) | leaf-local (Queue vs Library dest) | `[`/`]` rows |
| `handle_lib_key` Ctrl/Alt catch-all (6d) | library leaf local swallow | `Ctrl+z` row |
| Space/Escape double-tap (6e) | policy 4.3: first press `FallThrough`, second `Command` | double-tap rows |
| Ctrl+/ terminal-encoding ambiguity (6f) | policy overlay-open gate matches `Char('/')` OR `Char('_')` | Ctrl+/ row |

Every row in sections 1–6 maps to either a conversion unit (4–7), a deletion
(8.1), or a routing-matrix row (2.2). Nothing is silently dropped.