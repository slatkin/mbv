# Input Phase 2: Keyboard Precedence Spine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers-optimized:subagent-driven-development (recommended) or superpowers-optimized:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move `App::handle_key`'s ~315-line implicit branch-order body into an explicit, ordered, assertable `CONTEXT_STACK` of named handlers, so precedence is data you can read and test rather than control flow you have to mentally execute — with zero user-visible behavior change.

**Architecture:** Strangler-fig extraction, not a big-bang rewrite. Task 1 introduces `ContextEntry { name, handler }` and a `CONTEXT_STACK: &[ContextEntry]` in `src/app/input_resolver.rs`, wraps every branch/handler that is *already* its own method (the six `handle_key_*(&mut self, key) -> Option<bool>` overlay handlers, `handle_power_left_width_key`, `handle_key_context_menu`, `handle_playback_key`) as stack entries, and moves every remaining *inline* `if` block in `handle_key` verbatim into one temporary `handle_key_legacy_tail` entry near the end of the stack. `handle_key` itself becomes a single loop over `CONTEXT_STACK`. Tasks 2–6 each peel one inline group out of `handle_key_legacy_tail` into its own named, independently testable method and stack entry, shrinking the tail until it's empty and removed in Task 6. Task 7 pins the final stack order with a characterization test and folds in the phase-1-review follow-up (dead `InputSnapshot` construction for Help).

This is deliberately **not** a Command-ification of every context. Per `docs/adr/0002-centralized-input-handling.md`, text-entry and modal state machines (home/library search, playlists list, context menu) are *routed to* by the registry but keep their local state and `Char`-capture logic — only the *selection of which context handles this key* becomes explicit data. Purifying those handlers' internals into `Command` bindings is out of scope here (phase 3+ territory per the ADR roadmap, which covers view-handler collapse separately).

**Tech Stack:** Rust; `crossterm` (`KeyCode`, `KeyEvent`, `KeyModifiers`); existing `App` test scaffolding (`make_app_stub`, `Player::spy_on_commands`).

**Assumptions:**
- Assumes the current branch order in `handle_key` (lines 102–416 as of commit `2147343`) is the *correct*, currently-shipping behavior to preserve verbatim — this plan pins quirks, it does not fix them. Will NOT work as a vehicle for behavior changes; any bug noticed during extraction gets a comment (`// PRESERVED QUIRK: ...`) and a separate follow-up issue, not a fix here.
- Assumes `handle_combined_key`, `handle_lib_key`, and `handle_queue_key` (the three view-dispatch functions) are **not** touched internally — only the top-level `tab_idx` routing to them becomes one stack entry (`view_dispatch`). Their internal duplication (`tab_idx`-swap hack, `is_lib_key` mirror) is explicitly phase 3 per the ADR. Will NOT collapse duplicated global keys inside those functions.
- Assumes each extracted method keeps the exact guard conditions from the source so the extraction is representable as "no functional diff, only structural" — verified by re-running the full existing test suite after every task, not just the new characterization tests.

---

## File Structure

- Modify: `src/app/input_resolver.rs` — add `ContextEntry`, `CONTEXT_STACK`, and (Task 7) the Help-snapshot-skip fix.
- Modify: `src/app/input.rs` — `handle_key` becomes a stack loop; extracted methods added one group per task; `handle_key_legacy_tail` shrinks to nothing by Task 6.
- No new files beyond the existing `input_resolver.rs` from phase 1.

## Global Constraints

- Every commit MUST pass, from the repo root: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- Do NOT add `Co-Authored-By` trailers to commit messages.
- Do NOT push. Commit only.
- **Behavior-preserving:** no user-visible behavior change. Every task's verification step re-runs the *full* test suite (`cargo test`, not a filtered subset) — regressions in untouched contexts are exactly what a strangler-fig extraction can silently introduce if a guard is transcribed wrong.
- Existing quirks (e.g. Help "swallows every key", Playback's gate logic) are preserved **verbatim** and, where non-obvious, commented as intentional.
- New items are `pub(super)`, matching existing visibility.

---

### Task 1: `ContextEntry` scaffold + wrap existing modular handlers + legacy tail

**Files:**
- Modify: `src/app/input_resolver.rs`
- Modify: `src/app/input.rs`

**Security flag:** `none`

**Does NOT cover:** does not change any guard logic or ordering — this task only changes *how* `handle_key` is expressed (loop over data vs. a chain of `if let ... return`). It does not yet extract any of the inline `if` blocks (F-keys, confirms, `h`/`c`, search routing, Ctrl+L, F5, Alt+m) — those all move as one unit into `handle_key_legacy_tail`, unchanged, to be peeled apart in Tasks 2–6.

- [ ] **Step 1: Add `ContextEntry` and the stack type to `input_resolver.rs`**

Append to `src/app/input_resolver.rs` (after the phase-1 `resolve_key`/`input_snapshot` code, before the `#[cfg(test)]` block):

```rust
use crossterm::event::KeyEvent;

/// One layer of the keyboard precedence stack: a name for assertions/debugging
/// and a handler that returns `Some(quit)` if this context claimed the key, or
/// `None` to fall through to the next-lower-priority context. Phase 2 (#131)
/// makes `handle_key`'s branch order into this explicit, ordered, testable
/// list instead of implicit control flow.
#[derive(Clone, Copy)]
pub(super) struct ContextEntry {
    pub name: &'static str,
    pub handler: fn(&mut App, KeyEvent) -> Option<bool>,
}

/// The full keyboard context-priority stack, first-match-wins, in the exact
/// order `handle_key` checked them before phase 2. See
/// `docs/adr/0002-centralized-input-handling.md`.
pub(super) const CONTEXT_STACK: &[ContextEntry] = &[
    ContextEntry { name: "save_modal", handler: App::handle_key_save_modal },
    ContextEntry { name: "save_playlist", handler: App::handle_key_save_playlist_entry },
    ContextEntry { name: "settings", handler: App::handle_key_settings },
    ContextEntry { name: "help", handler: App::handle_key_help },
    ContextEntry { name: "sessions", handler: App::handle_key_sessions },
    ContextEntry { name: "playlists", handler: App::handle_key_playlists },
    ContextEntry { name: "legacy_tail", handler: App::handle_key_legacy_tail },
    ContextEntry { name: "context_menu", handler: App::handle_key_context_menu },
    ContextEntry { name: "playback", handler: App::handle_playback_key },
    ContextEntry { name: "view_dispatch", handler: App::handle_key_view_dispatch },
];
```

Note: `context_menu` and `playback` are listed *after* `legacy_tail` here deliberately — in the current source (`src/app/input.rs:365-398`), the `c`-clear-queue-prompt and confirm checks (which will live inside `legacy_tail` until Tasks 2-5 peel them out) run **before** `handle_key_context_menu`, which itself runs **before** `handle_playback_key`. Keeping `legacy_tail` as a single ordered block preserves that relative order exactly; only the outer wrapping changes.

- [ ] **Step 2: Wrap `handle_save_playlist_key` (currently unconditional `bool`) as an `Option<bool>` entry**

In `src/app/input.rs`, add (near `handle_save_playlist_key`'s existing definition):

```rust
fn handle_key_save_playlist_entry(&mut self, key: KeyEvent) -> Option<bool> {
    if self.save_playlist_dialog.is_some() {
        Some(self.handle_save_playlist_key(key))
    } else {
        None
    }
}
```

- [ ] **Step 3: Wrap `handle_power_left_width_key` (currently `bool`) — folded into `legacy_tail` for now**

This one stays inside `legacy_tail` in this task (it's an inline-checked-first branch, not yet its own stack entry); no new wrapper needed yet. Confirmed here so Step 4 below isn't ambiguous about where it lives.

- [ ] **Step 4: Extract `handle_key_legacy_tail` — move every remaining inline `if` block verbatim**

In `src/app/input.rs`, replace the body of `handle_key` (currently lines 102-416) with a new `handle_key_legacy_tail` containing everything from `self.handle_power_left_width_key(key)` (today's line 138) through the `c`-clear-queue-prompt block (today's line 384), **excluding** the F1-F4 global-open checks (today's lines 121-137, moved to `legacy_tail` too — see note below) and excluding `handle_key_context_menu`/`handle_playback_key`/Ctrl+L/F5/tab_idx-dispatch, which become their own entries (context_menu and playback already exist; Ctrl+L/F5/view_dispatch below):

```rust
fn handle_key_legacy_tail(&mut self, key: KeyEvent) -> Option<bool> {
    if key.code == KeyCode::F(1) {
        self.show_help = true;
        return Some(false);
    }
    if key.code == KeyCode::F(2) {
        self.show_settings = !self.show_settings;
        return Some(false);
    }
    if key.code == KeyCode::F(3) {
        self.show_sessions = true;
        self.spawn_sessions_load();
        return Some(false);
    }
    if key.code == KeyCode::F(4) {
        self.open_playlists_panel();
        return Some(false);
    }
    if self.handle_power_left_width_key(key) {
        return Some(false);
    }
    // Alt+Left/Right cycle type filter when home search is active
    if (self.tab_idx == 0 || self.tab_idx == 1)
        && key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && self.home_search.is_some()
        && self.context_menu.is_none()
    {
        match key.code {
            KeyCode::Left | KeyCode::Right => {
                if let Some(ref mut hs) = self.home_search {
                    let n = hs.available_types().len() + 1; // +1 for "All"
                    if n > 1 {
                        hs.type_filter = if key.code == KeyCode::Right {
                            (hs.type_filter + 1) % n
                        } else {
                            (hs.type_filter + n - 1) % n
                        };
                        hs.cursor = 0;
                        hs.scroll = 0;
                    }
                }
                return Some(false);
            }
            _ => {}
        }
    }
    // When home search is active, unmodified keys feed the search input
    if (self.tab_idx == 0 || self.tab_idx == 1)
        && !key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && self.home_search.is_some()
        && self.context_menu.is_none()
    {
        let input_focused = self.home_search.as_ref().is_none_or(|s| s.input_focused);
        match key.code {
            KeyCode::Esc => {
                self.home_search = None;
            }
            KeyCode::Tab => {
                if let Some(ref mut hs) = self.home_search {
                    hs.input_focused = !hs.input_focused;
                }
            }
            KeyCode::Backspace if input_focused => {
                let empty = self.home_search.as_ref().is_none_or(|s| s.query.is_empty());
                if empty {
                    self.home_search = None;
                } else {
                    self.home_search.as_mut().unwrap().query.pop();
                }
            }
            KeyCode::Up => {
                if let Some(ref mut hs) = self.home_search {
                    hs.cursor = hs.cursor.saturating_sub(1);
                    if hs.cursor < hs.scroll {
                        hs.scroll = hs.cursor;
                    }
                }
            }
            KeyCode::Down => {
                if let Some(ref mut hs) = self.home_search {
                    let max = hs.filtered_count().saturating_sub(1);
                    hs.cursor = (hs.cursor + 1).min(max);
                }
            }
            KeyCode::Enter => {
                let (query, last_query, loading, has_results) = self
                    .home_search
                    .as_ref()
                    .map(|hs| {
                        (
                            hs.query.clone(),
                            hs.last_query.clone(),
                            hs.loading,
                            !hs.results.is_empty(),
                        )
                    })
                    .unwrap_or_default();
                if loading {
                    return Some(false);
                }
                if !input_focused {
                    if has_results {
                        self.select_home();
                    }
                    return Some(false);
                }
                if query.is_empty() {
                    return Some(false);
                }
                if query != last_query {
                    if let Some(ref mut hs) = self.home_search {
                        hs.last_query = query.clone();
                        hs.loading = true;
                        hs.results.clear();
                        hs.cursor = 0;
                        hs.scroll = 0;
                    }
                    self.spawn_global_search(query);
                } else if has_results {
                    self.select_home();
                }
            }
            KeyCode::Char('q') if !input_focused && key.modifiers.is_empty() => {
                return Some(self.try_quit());
            }
            KeyCode::Char(c) => {
                if let Some(ref mut hs) = self.home_search {
                    hs.input_focused = true;
                    hs.query.push(c);
                }
            }
            _ => {}
        }
        return Some(false);
    }
    // Power-view: when the left panel is focused on a library with active search, intercept keys
    if self.queue_view == QUEUE_VIEW_POWER
        && !key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && self.context_menu.is_none()
        && matches!(self.power_focus, PowerFocus::Left)
        && self.power_left_tab > 0
    {
        let lib_idx = self.power_left_tab - 1;
        if self.libs[lib_idx].search.is_some() {
            self.handle_lib_search_key(lib_idx, key);
            return Some(false);
        }
    }
    // When library search is active, unmodified keys feed the search
    if self.tab_idx > 1
        && !key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && self
            .libs
            .get(self.tab_idx - self.lib_tab_offset())
            .is_some_and(|l| l.search.is_some())
        && self.context_menu.is_none()
    {
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        self.handle_lib_search_key(lib_idx, key);
        return Some(false);
    }
    if key.code == KeyCode::Char('h') {
        let active = self.player.status.lock().unwrap().active;
        let show_controls = active || self.connected_session_id.is_some();
        if show_controls {
            self.panel_mode = self.panel_mode.next();
        }
        return Some(false);
    }
    let in_lib_search = self.tab_idx > 1
        && self
            .libs
            .get(self.tab_idx - self.lib_tab_offset())
            .is_some_and(|l| l.search.is_some());
    if self.confirm_clear_queue {
        self.confirm_clear_queue = false;
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
        } else {
            self.status.clear();
        }
        return Some(false);
    }
    if self.confirm_rescan {
        self.confirm_rescan = false;
        self.status.clear();
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            self.trigger_lib_rescan(lib_idx);
        }
        return Some(false);
    }
    if self.skip_intro_end_ticks.is_some() {
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
                let secs = end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
                self.player.send_command(PlayerCommand::SeekAbsolute(secs));
                self.player.send_command(PlayerCommand::SkipIntroDismiss);
                self.status.clear();
            }
        } else {
            self.skip_intro_end_ticks = None;
            self.player.send_command(PlayerCommand::SkipIntroDismiss);
            self.status.clear();
        }
        return Some(false);
    }
    if self.next_up_item.is_some() {
        if matches!(
            key.code,
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
        ) {
            if let Some(item) = self.next_up_item.take() {
                if let Some(idx) = self
                    .playback_queue()
                    .items
                    .iter()
                    .position(|i| i.id == item.id)
                {
                    let label = item.playback_label();
                    self.player.send_command(PlayerCommand::JumpTo(idx));
                    self.playback_queue_mut().queue_cursor = idx;
                    self.flash_status(label);
                }
            }
        } else {
            self.next_up_item = None;
            self.player.send_command(PlayerCommand::NextUpDismiss);
            self.status.clear();
        }
        return Some(false);
    }
    if key.code == KeyCode::Char('c')
        && !key.modifiers.contains(KeyModifiers::ALT)
        && !in_lib_search
    {
        if self.tab_idx == 1 && self.visible_queue_scope() == QueueScope::Remote {
            self.flash_status_high("Remote queue is controlled by the daemon".into());
            return Some(false);
        }
        if self.player_tab.items.is_empty() {
            return Some(false);
        }
        self.notify_with_actions(
            "mbv",
            "Clear queue?",
            &[("clear:yes", "Clear"), ("clear:no", "Cancel")],
        );
        self.status = "Clear queue? (Y/n)".into();
        self.confirm_clear_queue = true;
        return Some(false);
    }
    None
}
```

- [ ] **Step 5: Add `handle_key_view_dispatch` (the tail) and Ctrl+L/F5 — kept inline in `legacy_tail` for this task**

Ctrl+L, F5, and the Alt+m power-queue branch (source lines 388-397, 401-407) still run *between* `context_menu` and `view_dispatch` in the current source. To keep Task 1 a pure structural move without reordering anything, append them to the **end** of `handle_key_legacy_tail` (after the `c` block, before its final `None`) exactly as in the source:

```rust
    if key.code == KeyCode::Char('m')
        && key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && self.tab_idx == 1
        && self.queue_view == QUEUE_VIEW_POWER
        && matches!(self.power_focus, PowerFocus::Left)
        && self.power_left_tab > 0
    {
        return Some(self.handle_queue_key(key));
    }
```

This must be inserted **after** the point where `context_menu`/`playback` would have already run in source order (source: `c`-block at 365-384, then `handle_key_context_menu` at 385-387, then Alt+m at 388-397, then `handle_playback_key` at 398-400). Since `legacy_tail` is one stack entry that runs *before* the `context_menu`/`playback` entries per Step 1's ordering note, moving Alt+m here would run it too early relative to `context_menu`. **Correction:** move the Alt+m block instead to `handle_key_view_dispatch` as a leading check, since it only fires when `context_menu` is already `None` (implied by `power_focus == Left`, unrelated to context menu) — but to stay strictly order-preserving, add it as a **new stack entry** positioned between `context_menu` and `playback`:

```rust
ContextEntry { name: "power_queue_alt_m", handler: App::handle_key_power_queue_alt_m },
```

placed in `CONTEXT_STACK` between `context_menu` and `playback`, with:

```rust
fn handle_key_power_queue_alt_m(&mut self, key: KeyEvent) -> Option<bool> {
    if key.code == KeyCode::Char('m')
        && key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && self.tab_idx == 1
        && self.queue_view == QUEUE_VIEW_POWER
        && matches!(self.power_focus, PowerFocus::Left)
        && self.power_left_tab > 0
    {
        Some(self.handle_queue_key(key))
    } else {
        None
    }
}
```

Similarly add `ctrl_l_force_clear` and `f5_refresh` as their own entries positioned between `playback` and `view_dispatch` (matching source lines 401-408, which run after `handle_playback_key` and before the `tab_idx` dispatch):

```rust
fn handle_key_ctrl_l(&mut self, key: KeyEvent) -> Option<bool> {
    if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
        self.force_clear = true;
        Some(false)
    } else {
        None
    }
}

fn handle_key_f5_refresh(&mut self, key: KeyEvent) -> Option<bool> {
    if key.code == KeyCode::F(5) {
        self.refresh_current_view();
        Some(false)
    } else {
        None
    }
}

fn handle_key_view_dispatch(&mut self, key: KeyEvent) -> Option<bool> {
    Some(if self.tab_idx == 0 {
        self.handle_combined_key(key)
    } else if self.tab_idx == 1 {
        self.handle_queue_key(key)
    } else {
        self.handle_lib_key(key)
    })
}
```

Final `CONTEXT_STACK` for this task, replacing Step 1's draft:

```rust
pub(super) const CONTEXT_STACK: &[ContextEntry] = &[
    ContextEntry { name: "save_modal", handler: App::handle_key_save_modal },
    ContextEntry { name: "save_playlist", handler: App::handle_key_save_playlist_entry },
    ContextEntry { name: "settings", handler: App::handle_key_settings },
    ContextEntry { name: "help", handler: App::handle_key_help },
    ContextEntry { name: "sessions", handler: App::handle_key_sessions },
    ContextEntry { name: "playlists", handler: App::handle_key_playlists },
    ContextEntry { name: "legacy_tail", handler: App::handle_key_legacy_tail },
    ContextEntry { name: "context_menu", handler: App::handle_key_context_menu },
    ContextEntry { name: "power_queue_alt_m", handler: App::handle_key_power_queue_alt_m },
    ContextEntry { name: "playback", handler: App::handle_playback_key },
    ContextEntry { name: "ctrl_l_force_clear", handler: App::handle_key_ctrl_l },
    ContextEntry { name: "f5_refresh", handler: App::handle_key_f5_refresh },
    ContextEntry { name: "view_dispatch", handler: App::handle_key_view_dispatch },
];
```

- [ ] **Step 6: Rewrite `handle_key` as a stack loop**

Replace the (now-vacated) body of `pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool` in `src/app/input.rs` with:

```rust
pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
    for entry in super::input_resolver::CONTEXT_STACK {
        if let Some(quit) = (entry.handler)(self, key) {
            return quit;
        }
    }
    false
}
```

- [ ] **Step 7: Add a stack-order characterization test**

Append to `src/app/input_resolver.rs`'s existing `#[cfg(test)]` block (or a new `mod stack_tests` if the existing one is scoped to pure resolver tests):

```rust
#[test]
fn context_stack_order_is_pinned() {
    let names: Vec<&str> = CONTEXT_STACK.iter().map(|e| e.name).collect();
    assert_eq!(
        names,
        vec![
            "save_modal",
            "save_playlist",
            "settings",
            "help",
            "sessions",
            "playlists",
            "legacy_tail",
            "context_menu",
            "power_queue_alt_m",
            "playback",
            "ctrl_l_force_clear",
            "f5_refresh",
            "view_dispatch",
        ],
        "precedence order must match handle_key's pre-phase-2 branch order; \
         if this intentionally changes, update docs/adr/0002-centralized-input-handling.md too"
    );
}
```

- [ ] **Step 8: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS. Every pre-existing test (help/playback/mouse/queue/library characterization tests) still green — this is the proof that the structural move changed nothing observable.

- [ ] **Step 9: Commit**

```bash
git add src/app/input_resolver.rs src/app/input.rs
git commit -m "Extract handle_key into an explicit CONTEXT_STACK with a legacy tail"
```

---

### Task 2: Peel out the four global overlay-open keys (F1–F4)

**Files:**
- Modify: `src/app/input.rs`

**Security flag:** `none`

- [ ] **Step 1: Write characterization tests (red — these pass today via legacy_tail, must still pass after)**

Add to `src/app/input_resolver.rs`'s `app_level_tests` module (or wherever phase-1's `handle_key`-level tests live):

```rust
#[test]
fn f1_opens_help_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(1),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.show_help);
}

#[test]
fn f2_toggles_settings_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    assert!(!app.show_settings);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(2),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.show_settings);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(2),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(!app.show_settings, "F2 toggles, not just opens");
}
```

- [ ] **Step 2: Run to confirm both already pass (they do, via `legacy_tail`)**

Run: `cargo test f1_opens_help_via_handle_key f2_toggles_settings_via_handle_key`
Expected: PASS (baseline, proving current behavior before extraction).

- [ ] **Step 3: Extract `handle_key_global_overlay_open`**

In `src/app/input.rs`, add:

```rust
fn handle_key_global_overlay_open(&mut self, key: KeyEvent) -> Option<bool> {
    if key.code == KeyCode::F(1) {
        self.show_help = true;
        return Some(false);
    }
    if key.code == KeyCode::F(2) {
        self.show_settings = !self.show_settings;
        return Some(false);
    }
    if key.code == KeyCode::F(3) {
        self.show_sessions = true;
        self.spawn_sessions_load();
        return Some(false);
    }
    if key.code == KeyCode::F(4) {
        self.open_playlists_panel();
        return Some(false);
    }
    None
}
```

Remove the four `if key.code == KeyCode::F(N)` blocks from the top of `handle_key_legacy_tail` (they're the first four checks in Task 1's Step 4 body).

- [ ] **Step 4: Register the new entry, ahead of `legacy_tail`, in `input_resolver.rs`'s `CONTEXT_STACK`**

```rust
ContextEntry { name: "global_overlay_open", handler: App::handle_key_global_overlay_open },
```

placed immediately before `legacy_tail`.

- [ ] **Step 5: Update the pinned-order test**

In `context_stack_order_is_pinned` (Task 1, Step 7), insert `"global_overlay_open"` immediately before `"legacy_tail"` in the expected `vec![...]`.

- [ ] **Step 6: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, including the two new tests and the updated `context_stack_order_is_pinned`.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs src/app/input_resolver.rs
git commit -m "Peel F1-F4 global overlay opens out of the legacy tail"
```

---

### Task 3: Peel out the four transient confirms

**Files:**
- Modify: `src/app/input.rs`

**Security flag:** `none`

**Does NOT cover:** the `y`/`Y`/`Enter` semantics or the notification/status side effects for each confirm — those are copied verbatim from source; this task only changes which stack entry claims the key.

- [ ] **Step 1: Write characterization tests**

```rust
#[test]
fn confirm_clear_queue_yes_dispatches_clear_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.confirm_clear_queue = true;
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('y'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(!app.confirm_clear_queue, "confirm flag clears regardless of answer");
}

#[test]
fn confirm_rescan_no_clears_flag_without_rescan_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.confirm_rescan = true;
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('n'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(!app.confirm_rescan);
}

#[test]
fn skip_intro_confirm_no_dismisses_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.skip_intro_end_ticks = Some(1000);
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('n'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.skip_intro_end_ticks.is_none());
}
```

- [ ] **Step 2: Run to confirm they pass today (baseline via `legacy_tail`)**

Run: `cargo test confirm_clear_queue_yes confirm_rescan_no skip_intro_confirm_no`
Expected: PASS.

- [ ] **Step 3: Extract the four confirm handlers**

In `src/app/input.rs`:

```rust
fn handle_key_confirm_clear_queue(&mut self, key: KeyEvent) -> Option<bool> {
    if !self.confirm_clear_queue {
        return None;
    }
    self.confirm_clear_queue = false;
    if matches!(
        key.code,
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
    ) {
        self.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
    } else {
        self.status.clear();
    }
    Some(false)
}

fn handle_key_confirm_rescan(&mut self, key: KeyEvent) -> Option<bool> {
    if !self.confirm_rescan {
        return None;
    }
    self.confirm_rescan = false;
    self.status.clear();
    if matches!(
        key.code,
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
    ) {
        let lib_idx = self.tab_idx - self.lib_tab_offset();
        self.trigger_lib_rescan(lib_idx);
    }
    Some(false)
}

fn handle_key_confirm_skip_intro(&mut self, key: KeyEvent) -> Option<bool> {
    if self.skip_intro_end_ticks.is_none() {
        return None;
    }
    if matches!(
        key.code,
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
    ) {
        if let Some(end_ticks) = self.skip_intro_end_ticks.take() {
            let secs = end_ticks as f64 / mbv_core::api::TICKS_PER_SECOND as f64;
            self.player.send_command(PlayerCommand::SeekAbsolute(secs));
            self.player.send_command(PlayerCommand::SkipIntroDismiss);
            self.status.clear();
        }
    } else {
        self.skip_intro_end_ticks = None;
        self.player.send_command(PlayerCommand::SkipIntroDismiss);
        self.status.clear();
    }
    Some(false)
}

fn handle_key_confirm_next_up(&mut self, key: KeyEvent) -> Option<bool> {
    if self.next_up_item.is_none() {
        return None;
    }
    if matches!(
        key.code,
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
    ) {
        if let Some(item) = self.next_up_item.take() {
            if let Some(idx) = self
                .playback_queue()
                .items
                .iter()
                .position(|i| i.id == item.id)
            {
                let label = item.playback_label();
                self.player.send_command(PlayerCommand::JumpTo(idx));
                self.playback_queue_mut().queue_cursor = idx;
                self.flash_status(label);
            }
        }
    } else {
        self.next_up_item = None;
        self.player.send_command(PlayerCommand::NextUpDismiss);
        self.status.clear();
    }
    Some(false)
}
```

Remove the corresponding four `if` blocks from `handle_key_legacy_tail` (they sit between the `h`-toggle block and the `c`-clear-queue-prompt block in Task 1's Step 4 body).

- [ ] **Step 4: Register the four entries in order, replacing their old position in `legacy_tail`**

In `CONTEXT_STACK`, insert (immediately after `legacy_tail`'s new, shorter position — see Step 5 note) in this exact relative order: `confirm_clear_queue`, `confirm_rescan`, `confirm_skip_intro`, `confirm_next_up`. Since these ran *after* the `h` block and *before* the `c` block in `legacy_tail`, and `legacy_tail` remains a single entry containing the `h` block, the `c` block, home/lib search, and power-left-width, the four confirms must be spliced into `CONTEXT_STACK` **inside** `legacy_tail`'s old position — i.e. `legacy_tail` now ends right after the `h`-toggle block, and a new `legacy_tail_2` entry (or simply keep one `legacy_tail` and additionally extract the trailing part) picks up from the `c` block onward. To avoid a confusing split name, rename: the first remaining chunk stays `legacy_tail` (power-left-width through `h`-toggle), and add `legacy_tail_confirms_done` → actually, simplest: keep **one** `handle_key_legacy_tail` covering power-left-width/home-search/lib-search/`h`/`c`/Alt+m-adjacent bits, and splice the four new confirm entries into `CONTEXT_STACK` at the exact point they used to run — between two halves of the tail. Concretely:

  - Split `handle_key_legacy_tail` into `handle_key_legacy_tail_pre_confirms` (power-left-width through the `h`-toggle block) and `handle_key_legacy_tail_post_confirms` (the `in_lib_search` let-binding plus the `c`-clear-queue-prompt block only — Alt+m/Ctrl+L/F5 already moved to their own entries in Task 1).
  - `CONTEXT_STACK` order becomes: `...playlists, global_overlay_open, legacy_tail_pre_confirms, confirm_clear_queue, confirm_rescan, confirm_skip_intro, confirm_next_up, legacy_tail_post_confirms, context_menu, power_queue_alt_m, playback, ctrl_l_force_clear, f5_refresh, view_dispatch`.

- [ ] **Step 5: Update the pinned-order test**

Replace `"legacy_tail"` in `context_stack_order_is_pinned`'s expected vec with the six entries from Step 4's final order.

- [ ] **Step 6: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs src/app/input_resolver.rs
git commit -m "Peel the four transient-confirm handlers out of the legacy tail"
```

---

### Task 4: Peel out home search and library search routing

**Files:**
- Modify: `src/app/input.rs`

**Security flag:** `none`

**Does NOT cover:** the internal search-state machine (query editing, cursor movement, debounced spawn) — that logic is copied verbatim; only its selection as a context becomes explicit. Per the ADR, text-entry contexts stay local/`Char`-capture-based, not bindings.

- [ ] **Step 1: Write characterization tests**

```rust
#[test]
fn home_search_captures_char_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.home_search = Some(Default::default());
    if let Some(hs) = app.home_search.as_mut() {
        hs.input_focused = true;
    }
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('x'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.home_search.as_ref().unwrap().query, "x");
}

#[test]
fn home_search_esc_closes_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.home_search = Some(Default::default());
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.home_search.is_none());
}
```

(If `HomeSearch` does not derive `Default`, construct it via whatever constructor the existing home-search tests elsewhere in `input.rs` already use — check with `find_symbol` on `HomeSearch` before writing this step's final code; do not guess the constructor.)

- [ ] **Step 2: Run to confirm they pass today**

Run: `cargo test home_search_captures_char_via_handle_key home_search_esc_closes_via_handle_key`
Expected: PASS.

- [ ] **Step 3: Extract `handle_key_home_search` (Alt-cycle + char-capture, combined — same guard family)**

```rust
fn handle_key_home_search(&mut self, key: KeyEvent) -> Option<bool> {
    if !(self.tab_idx == 0 || self.tab_idx == 1)
        || self.home_search.is_none()
        || self.context_menu.is_some()
    {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL)
    {
        match key.code {
            KeyCode::Left | KeyCode::Right => {
                if let Some(ref mut hs) = self.home_search {
                    let n = hs.available_types().len() + 1;
                    if n > 1 {
                        hs.type_filter = if key.code == KeyCode::Right {
                            (hs.type_filter + 1) % n
                        } else {
                            (hs.type_filter + n - 1) % n
                        };
                        hs.cursor = 0;
                        hs.scroll = 0;
                    }
                }
                return Some(false);
            }
            _ => return None,
        }
    }
    if key.modifiers.contains(KeyModifiers::ALT) || key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    let input_focused = self.home_search.as_ref().is_none_or(|s| s.input_focused);
    match key.code {
        KeyCode::Esc => {
            self.home_search = None;
        }
        KeyCode::Tab => {
            if let Some(ref mut hs) = self.home_search {
                hs.input_focused = !hs.input_focused;
            }
        }
        KeyCode::Backspace if input_focused => {
            let empty = self.home_search.as_ref().is_none_or(|s| s.query.is_empty());
            if empty {
                self.home_search = None;
            } else {
                self.home_search.as_mut().unwrap().query.pop();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut hs) = self.home_search {
                hs.cursor = hs.cursor.saturating_sub(1);
                if hs.cursor < hs.scroll {
                    hs.scroll = hs.cursor;
                }
            }
        }
        KeyCode::Down => {
            if let Some(ref mut hs) = self.home_search {
                let max = hs.filtered_count().saturating_sub(1);
                hs.cursor = (hs.cursor + 1).min(max);
            }
        }
        KeyCode::Enter => {
            let (query, last_query, loading, has_results) = self
                .home_search
                .as_ref()
                .map(|hs| {
                    (
                        hs.query.clone(),
                        hs.last_query.clone(),
                        hs.loading,
                        !hs.results.is_empty(),
                    )
                })
                .unwrap_or_default();
            if loading {
                return Some(false);
            }
            if !input_focused {
                if has_results {
                    self.select_home();
                }
                return Some(false);
            }
            if query.is_empty() {
                return Some(false);
            }
            if query != last_query {
                if let Some(ref mut hs) = self.home_search {
                    hs.last_query = query.clone();
                    hs.loading = true;
                    hs.results.clear();
                    hs.cursor = 0;
                    hs.scroll = 0;
                }
                self.spawn_global_search(query);
            } else if has_results {
                self.select_home();
            }
        }
        KeyCode::Char('q') if !input_focused && key.modifiers.is_empty() => {
            return Some(self.try_quit());
        }
        KeyCode::Char(c) => {
            if let Some(ref mut hs) = self.home_search {
                hs.input_focused = true;
                hs.query.push(c);
            }
        }
        _ => {}
    }
    Some(false)
}
```

- [ ] **Step 4: Extract `handle_key_power_lib_search` and `handle_key_lib_search`**

```rust
fn handle_key_power_lib_search(&mut self, key: KeyEvent) -> Option<bool> {
    if self.queue_view != QUEUE_VIEW_POWER
        || key.modifiers.contains(KeyModifiers::ALT)
        || key.modifiers.contains(KeyModifiers::CONTROL)
        || self.context_menu.is_some()
        || !matches!(self.power_focus, PowerFocus::Left)
        || self.power_left_tab == 0
    {
        return None;
    }
    let lib_idx = self.power_left_tab - 1;
    if self.libs[lib_idx].search.is_some() {
        self.handle_lib_search_key(lib_idx, key);
        Some(false)
    } else {
        None
    }
}

fn handle_key_lib_search(&mut self, key: KeyEvent) -> Option<bool> {
    if self.tab_idx <= 1
        || key.modifiers.contains(KeyModifiers::ALT)
        || key.modifiers.contains(KeyModifiers::CONTROL)
        || self.context_menu.is_some()
    {
        return None;
    }
    if !self
        .libs
        .get(self.tab_idx - self.lib_tab_offset())
        .is_some_and(|l| l.search.is_some())
    {
        return None;
    }
    let lib_idx = self.tab_idx - self.lib_tab_offset();
    self.handle_lib_search_key(lib_idx, key);
    Some(false)
}
```

Remove the corresponding blocks from `handle_key_legacy_tail_pre_confirms` (Task 3): the Alt-cycle block, the home-search-char-capture block, the power-lib-search-intercept block, and the regular lib-search block.

- [ ] **Step 5: Register the three new entries, in the same relative order, replacing that portion of `legacy_tail_pre_confirms`**

`CONTEXT_STACK` order for this stretch becomes: `..., global_overlay_open, home_search, power_lib_search, lib_search, legacy_tail_pre_confirms (now just power-left-width + h-toggle), confirm_clear_queue, ...`. Note `handle_power_left_width_key` ran *before* the Alt-cycle/home-search block in source, so `legacy_tail_pre_confirms` (power-left-width + `h`) must stay listed **before** `home_search`/`power_lib_search`/`lib_search` — i.e. the actual final order is: `..., global_overlay_open, legacy_tail_power_width_and_h, home_search, power_lib_search, lib_search, confirm_clear_queue, confirm_rescan, confirm_skip_intro, confirm_next_up, legacy_tail_post_confirms, context_menu, power_queue_alt_m, playback, ctrl_l_force_clear, f5_refresh, view_dispatch`. Rename `legacy_tail_pre_confirms` to `legacy_tail_power_width_and_h` for clarity (it now only wraps `handle_power_left_width_key` and the `h`-toggle).

- [ ] **Step 6: Update the pinned-order test to the full 17-entry order from Step 5**

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/app/input.rs src/app/input_resolver.rs
git commit -m "Peel home and library search routing out of the legacy tail"
```

---

### Task 5: Peel out `c` clear-queue-prompt; delete the legacy tail

> **Revised after Task 4.** Task 4's self-review caught an ordering bug baked into this task's original text (see below): the `h`-toggle had been grouped with `handle_power_left_width_key` into one `legacy_tail_power_width_and_h` fragment positioned *before* `home_search`/`power_lib_search`/`lib_search` in `CONTEXT_STACK`. But in the actual pre-phase-2 source, the `h`-toggle ran *after* all three search blocks (source order: power-left-width, home-search Alt-cycle, home-search char-capture, power-lib-search, lib-search, `h`-toggle, confirms...). Leaving `h` misplaced — even temporarily, between tasks — would ship a real behavior change (pressing 'h' while a search box is focused would toggle the panel instead of being captured as a character), so it was fixed immediately as part of Task 4 rather than carried forward: `handle_key_legacy_tail_power_width_and_h` was split into `handle_key_legacy_tail_power_width` (stays before `home_search`) and a standalone `handle_key_panel_toggle` (moved to its correct source-order slot, after `lib_search`), with a regression test (`home_search_char_capture_wins_over_h_panel_toggle_via_handle_key`) pinning the precedence. **`h` is therefore already done — this task now covers only `c`.**

**Files:**
- Modify: `src/app/input.rs`
- Modify: `src/app/input_resolver.rs`

**Security flag:** `none`

- [ ] **Step 1: Write a characterization test**

```rust
#[test]
fn c_prompts_clear_queue_confirmation_via_handle_key() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items.push(crate::app::tests::make_item("1", "Track"));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('c'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.confirm_clear_queue);
}
```

Use whatever `MediaItem` test-fixture helper Tasks 3/4 already established in this file (Task 3 used `crate::app::tests::make_item`) rather than assuming `MediaItem` derives `Default` — verify the exact helper name/signature already in use in this worktree before finalizing this step, since it may differ slightly from the placeholder call above.

- [ ] **Step 2: Run to confirm it passes today**

Run: `cargo test c_prompts_clear_queue_confirmation_via_handle_key`
Expected: PASS.

- [ ] **Step 3: Extract `handle_key_clear_queue_prompt`; delete `handle_key_legacy_tail_post_confirms`**

```rust
fn handle_key_clear_queue_prompt(&mut self, key: KeyEvent) -> Option<bool> {
    if key.code != KeyCode::Char('c') || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    let in_lib_search = self.tab_idx > 1
        && self
            .libs
            .get(self.tab_idx - self.lib_tab_offset())
            .is_some_and(|l| l.search.is_some());
    if in_lib_search {
        return None;
    }
    if self.tab_idx == 1 && self.visible_queue_scope() == QueueScope::Remote {
        self.flash_status_high("Remote queue is controlled by the daemon".into());
        return Some(false);
    }
    if self.player_tab.items.is_empty() {
        return Some(false);
    }
    self.notify_with_actions(
        "mbv",
        "Clear queue?",
        &[("clear:yes", "Clear"), ("clear:no", "Cancel")],
    );
    self.status = "Clear queue? (Y/n)".into();
    self.confirm_clear_queue = true;
    Some(false)
}
```

This is the entire remaining body of `handle_key_legacy_tail_post_confirms` (the `in_lib_search` binding plus the `c`-clear-queue-prompt block) — after this extraction, `handle_key_legacy_tail_post_confirms` has nothing left in it and is deleted entirely, along with its `CONTEXT_STACK` entry.

- [ ] **Step 4: Final `CONTEXT_STACK`**

```rust
pub(super) const CONTEXT_STACK: &[ContextEntry] = &[
    ContextEntry { name: "save_modal", handler: App::handle_key_save_modal },
    ContextEntry { name: "save_playlist", handler: App::handle_key_save_playlist_entry },
    ContextEntry { name: "settings", handler: App::handle_key_settings },
    ContextEntry { name: "help", handler: App::handle_key_help },
    ContextEntry { name: "sessions", handler: App::handle_key_sessions },
    ContextEntry { name: "playlists", handler: App::handle_key_playlists },
    ContextEntry { name: "global_overlay_open", handler: App::handle_key_global_overlay_open },
    ContextEntry { name: "legacy_tail_power_width", handler: App::handle_key_legacy_tail_power_width },
    ContextEntry { name: "home_search", handler: App::handle_key_home_search },
    ContextEntry { name: "power_lib_search", handler: App::handle_key_power_lib_search },
    ContextEntry { name: "lib_search", handler: App::handle_key_lib_search },
    ContextEntry { name: "panel_toggle_h", handler: App::handle_key_panel_toggle },
    ContextEntry { name: "confirm_clear_queue", handler: App::handle_key_confirm_clear_queue },
    ContextEntry { name: "confirm_rescan", handler: App::handle_key_confirm_rescan },
    ContextEntry { name: "confirm_skip_intro", handler: App::handle_key_confirm_skip_intro },
    ContextEntry { name: "confirm_next_up", handler: App::handle_key_confirm_next_up },
    ContextEntry { name: "clear_queue_prompt_c", handler: App::handle_key_clear_queue_prompt },
    ContextEntry { name: "context_menu", handler: App::handle_key_context_menu },
    ContextEntry { name: "power_queue_alt_m", handler: App::handle_key_power_queue_alt_m },
    ContextEntry { name: "playback", handler: App::handle_playback_key },
    ContextEntry { name: "ctrl_l_force_clear", handler: App::handle_key_ctrl_l },
    ContextEntry { name: "f5_refresh", handler: App::handle_key_f5_refresh },
    ContextEntry { name: "view_dispatch", handler: App::handle_key_view_dispatch },
];
```

Note `legacy_tail_power_width` (renamed from the mid-Task-4-fix `handle_key_legacy_tail_power_width`, wrapping only `handle_power_left_width_key` now) and `panel_toggle_h` are already in place from Task 4's fix, in their correct (source-verified) relative positions — this task only adds `clear_queue_prompt_c` and removes `legacy_tail_post_confirms`. Optionally rename `legacy_tail_power_width` to a non-"legacy_tail"-prefixed name (e.g. `power_left_width`) as part of this task's cleanup, since it's no longer a strangler-fig remnant sharing space with anything else — it's already a clean, single-purpose wrapper.

- [ ] **Step 5: Update the pinned-order test to this final 23-entry list**

- [ ] **Step 6: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS. `handle_key_legacy_tail_post_confirms` no longer exists anywhere (`grep -rn "legacy_tail_post_confirms" src/` returns nothing). `legacy_tail_power_width` (or its renamed equivalent) is the only survivor of the original `legacy_tail` split, and it wraps exactly one call (`handle_power_left_width_key`) — confirm this is intentional and not a further-splittable remnant before moving to Task 6.

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs src/app/input_resolver.rs
git commit -m "Peel c clear-queue-prompt out; delete the last legacy tail fragment"
```

---

### Task 6: Fold in the phase-1-review follow-up (dead Help `InputSnapshot`)

**Files:**
- Modify: `src/app/input_resolver.rs`
- Modify: `src/app/input.rs`

**Security flag:** `none`

Carried over from the #131 issue comment: `handle_key_help` builds an `InputSnapshot` every keystroke even though `resolve_key(InputContext::Help, ...)` never reads it (`help_command_for_key` ignores `player_active`/`has_remote_session`), taking the `player.status` lock for no reason on every help-overlay keypress.

- [ ] **Step 1: Write a test proving Help resolution doesn't need a snapshot**

```rust
#[test]
fn help_context_resolution_ignores_snapshot_fields() {
    // Two snapshots that differ in every field must resolve identically for Help,
    // proving Help's resolve_key path has no snapshot dependency.
    let a = InputSnapshot { player_active: true, has_remote_session: true };
    let b = InputSnapshot { player_active: false, has_remote_session: false };
    let chord = KeyChord::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        resolve_key(InputContext::Help, &a, chord),
        resolve_key(InputContext::Help, &b, chord)
    );
}
```

- [ ] **Step 2: Run to confirm it passes today (it does — this documents current behavior before the fix)**

Run: `cargo test help_context_resolution_ignores_snapshot_fields`
Expected: PASS.

- [ ] **Step 3: Restructure `resolve_key`'s `Help` arm to not require a snapshot; update the signature**

In `src/app/input_resolver.rs`, change `resolve_key` so contexts that don't consult the snapshot don't force callers to build one. Since other contexts (Playback, and now several of Task 2-5's contexts feeding through `dispatch` directly rather than `resolve_key`) still need it, the fix scoped to what phase 1 actually built is: make `InputSnapshot` construction lazy at the one remaining `resolve_key` call site that doesn't need it (`handle_key_help`).

In `src/app/input.rs`, change `handle_key_help`:

```rust
fn handle_key_help(&mut self, key: KeyEvent) -> Option<bool> {
    if !self.show_help {
        return None;
    }
    match super::input_resolver::help_resolve(super::input_resolver::KeyChord::from_key(key)) {
        super::input_resolver::KeyResolution::Command(cmd) => Some(self.dispatch(cmd)),
        super::input_resolver::KeyResolution::Swallow
        | super::input_resolver::KeyResolution::FallThrough => Some(false),
    }
}
```

In `src/app/input_resolver.rs`, add a snapshot-free entry point next to `resolve_key` and have `resolve_key`'s `Help` arm delegate to it (so both paths share one implementation, no drift):

```rust
/// Resolve a chord for the Help context. Does not need an `InputSnapshot` —
/// `help_command_for_key` ignores player/session state — so callers that only
/// ever hit this context (e.g. `handle_key_help`) can skip building one.
pub(super) fn help_resolve(chord: KeyChord) -> KeyResolution {
    match super::action::help_command_for_key(chord) {
        Some(cmd) => KeyResolution::Command(cmd),
        None => KeyResolution::Swallow,
    }
}
```

Update `resolve_key`'s `InputContext::Help` arm to call it:

```rust
InputContext::Help => help_resolve(chord),
```

- [ ] **Step 4: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS. `help_context_resolution_ignores_snapshot_fields` still passes (behavior unchanged); `handle_key_help` no longer takes `self.player.status.lock()` on every keystroke while help is open (verify by inspection — no lock call remains in the new body).

- [ ] **Step 5: Commit**

```bash
git add src/app/input_resolver.rs src/app/input.rs
git commit -m "Skip InputSnapshot construction for Help; it never reads one"
```

---

### Task 7: Final characterization sweep and docs

**Files:**
- Modify: `docs/adr/0002-centralized-input-handling.md` (mark phase 2 done in the roadmap list)
- Modify: `CONTEXT.md` (if it tracks phase status under "Input handling" — check first)

**Security flag:** `none`

- [ ] **Step 1: Add a handful of cross-context precedence tests that specifically exercise ordering (not just single-context behavior)**

```rust
#[test]
fn context_menu_open_blocks_home_search_char_capture_via_handle_key() {
    // Regression guard for the ADR's stated precedence: context_menu is not
    // actually checked by home_search's guard (`self.context_menu.is_none()`),
    // so an open context menu must swallow the key before home_search sees it.
    let mut app = crate::app::tests::make_app_stub();
    app.home_search = Some(Default::default());
    app.context_menu = Some(Default::default());
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('x'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(
        app.home_search.as_ref().unwrap().query.is_empty(),
        "context menu takes precedence; home search must not capture 'x'"
    );
}
```

(If `ContextMenu` doesn't derive `Default`, use `find_symbol` on `ContextMenu` to get its real constructor before finalizing — do not guess.)

- [ ] **Step 2: Run the full gate one final time**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, full suite, including every test added across Tasks 1–7.

- [ ] **Step 3: Update the ADR roadmap**

In `docs/adr/0002-centralized-input-handling.md`, change item 2 of the "Phased roadmap" list from a plain description to note completion, e.g. prefix with `**Done (#131).**` — follow whatever convention item 1 uses once phase 1 was marked complete (check if item 1 was annotated when #130/#141 merged; match that style rather than inventing a new one).

- [ ] **Step 4: Commit**

```bash
git add docs/adr/0002-centralized-input-handling.md
git commit -m "Mark input-handling phase 2 (keyboard precedence spine) complete"
```

---

## Self-Review

**Spec coverage (issue #131):**
- "Implement every keyboard context in the resolver: overlays... transient confirms, text-entry routing, context menu, globals..., and view dispatch" → every one of these becomes a named `CONTEXT_STACK` entry across Tasks 1–5. ✓
- "App-level characterization tests through `handle_key`; existing quirks pinned verbatim" → every task's Step 1 adds a `handle_key`-level test before extracting; Task 1's `context_stack_order_is_pinned` plus Task 7's cross-context test pin ordering specifically, which is the part plain per-context tests can't catch. ✓
- "Text-entry and modal state machines are routed to, not expressed as bindings" → `handle_key_home_search`/`handle_key_lib_search`/`handle_key_power_lib_search`/`handle_key_playlists` keep their local `Char`-capture/state-machine bodies; only their *selection* becomes a stack entry. ✓
- "No behavior change" → every task re-runs the full suite, not a filtered one; guard conditions are transcribed, never rewritten, from the verbatim source captured in Task 1 Step 4. ✓
- Carried-over follow-up (dead Help `InputSnapshot`) → Task 6. ✓

**Placeholder scan:** No TBD/TODO. The two "check the real constructor via `find_symbol` before finalizing" notes (Tasks 4, 5, 7) are concrete verification instructions, not deferred work — they exist because this plan was written without running the test suite against the live `HomeSearch`/`MediaItem`/`ContextMenu` types, and guessing a `Default` impl that doesn't exist would produce a red step for the wrong reason.

**Type consistency:** `ContextEntry { name, handler }` used identically from Task 1 onward. `fn(&mut App, KeyEvent) -> Option<bool>` signature is consistent across every extracted handler in every task. `CONTEXT_STACK`'s entry list is restated in full at the end of Tasks 1 and 5 (not just diffed) specifically so a reader / implementer never has to mentally merge partial edits across tasks.

**Ordering-fidelity risk:** this is the plan's biggest risk, flagged explicitly in Task 5 Step 4 ("re-derive the order from Task 1 Step 4 if in doubt") — several tasks reposition entries relative to each other as they're extracted, and a transposition would be a silent, hard-to-notice regression. The `context_stack_order_is_pinned` test (updated at the end of every task) is the guardrail; treat any failure of that test during implementation as more informative than the diff — it's rebuilt from the *previous* task's already-verified order each time, not from a fresh reading of old `handle_key`.

**Scope-reduction scan:** no "v1"/"basic"/"for now" language. The Assumptions section explicitly excludes view-handler internal collapse (phase 3) and Command-ification of text-entry internals (per ADR) — these are principled scope boundaries stated in the issue/ADR, not quiet downgrades.
