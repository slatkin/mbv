# TUI Status Bar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `mbv`'s TUI a persistent, always-on bottom status bar that anchors the screen, absorbs the existing transient toast mechanism, and surfaces session/connection and queue state that currently has no (or low-signal) on-screen representation.

**Architecture:** Repurpose a row slot that already exists in `App::render`'s vertical `Layout` but is currently always zero-height and unused (`_status_area` / `status_h`, `src/app/render/mod.rs:70-87`). Make it always `Constraint::Length(1)`. Relocate the control pill (`render_control_pill`, currently far-left of the tab row) into this row, add a session/connection label next to it, add a right-aligned queue-state segment (Queue tab / Power View only), and re-point the existing toast mechanism (`self.status` / `toast_line`) to render full-width in this row instead of a 3-row overlay that covers `main_area`.

**Tech Stack:** Rust, ratatui (`Layout`, `Paragraph`, `Span`, `Line`), existing `palette` color constants, `TestBackend`-based rendering tests (pattern already established in `src/app/render/mod.rs` and `src/app/mod.rs`).

## Global Constraints

- Design doc: `docs/superpowers/specs/2026-07-13-tui-status-bar-design.md`. Every task here implements a piece of that design; do not add scope beyond it (no breadcrumbs/nav context in the bar, no moving the res/audio/sub chips or `VOL` badge).
- Follow `AGENTS.md`: surgical changes only, no unrelated refactors, no speculative config/flexibility.
- `src/app/render/mod.rs` methods on `App` are private (`impl App` inside the `render` module, a child of `app`). Private items defined directly in `src/app/mod.rs` (fields, `RemoteSlotState`, `QueueScope`, `remote_slot_state()`, `visible_queue_scope()`) are already reachable from `render/mod.rs` via `super::` — this pattern is already used by `render_control_pill` (see `super::RemoteSlotState::Off` at `src/app/render/mod.rs`). Do not add `pub` to anything to work around a visibility error; if you hit one, you used the wrong path, not the wrong modifier.
- Run `cargo test --lib` after each task; run `cargo build` if you're unsure a change compiles before writing a test against it.

---

### Task 1: Reclaim the dormant status row and relocate the control pill into it

**Files:**
- Modify: `src/app/render/mod.rs:57-87` (the `status_h` / layout-array block inside `App::render`)
- Modify: `src/app/render/mod.rs:108-121` (the block that currently calls `self.render_control_pill(f, tabs_area, ...)`)
- Modify: `src/app/render/mod.rs` — `render_control_pill`'s doc comment (line ~737), since it no longer describes the tab row
- Test: `src/app/mod.rs` (existing `tests` module, alongside `render_app_to_string`-based tests)

**Interfaces:**
- Consumes: `AppLayout::playback: LayoutPlayback` (`src/app/layout.rs`), specifically `ind_mu: Rect` / `ind_rc: Rect`, already written by `render_control_pill`.
- Produces: a `status_bar_area: Rect` local variable in `App::render`, always 1 row tall, at the bottom of the screen (directly above nothing — it's the last row). Later tasks render into this same `status_bar_area`.

Today, `App::render` builds this array (verified via Serena read of the live source):

```rust
let tabs_h: u16 = 1;
let spacer_h: u16 = 1;
// seek = full-width seekbar row; title = now-playing row; controls = blank spacer below it. (status is unused.)
let (seek_h, gap_h, title_h, controls_h, status_h): (u16, u16, u16, u16, u16) =
    if onerow || reserve_player_rows {
        (1, 0, 1, 1, 0)
    } else {
        (1, 0, 0, 0, 0)
    };
let [tabs_area, _spacer_area, seek_area, _gap_area, title_area, _controls_area, _status_area, main_area] =
    Layout::vertical([
        Constraint::Length(tabs_h),
        Constraint::Length(spacer_h),
        Constraint::Length(seek_h),
        Constraint::Length(gap_h),
        Constraint::Length(title_h),
        Constraint::Length(controls_h),
        Constraint::Length(status_h),
        Constraint::Min(0),
    ])
    .areas(area);
```

`status_h` is `0` in both branches, so `_status_area` is always zero-height and unused — this is the exact "no row anchors the bottom" gap from the design doc, already half-scaffolded.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/app/mod.rs` (near the other `render_app_to_string` tests, e.g. after `attached_session_only_queue_has_no_scope_affordance_or_remote_switch`):

```rust
    #[test]
    fn status_bar_row_is_always_present_and_holds_the_control_pill() {
        let mut app = make_app_stub();
        app.tab_idx = 0; // Home tab, nothing playing — the row must still appear.

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains('\u{2261}'),
            "expected the control pill's playlist glyph (≡) on the final screen row:\n{rendered}"
        );
        // The pill must no longer render inside the tab row (first line).
        let first_line = rendered.lines().next().unwrap();
        assert!(
            !first_line.contains('\u{2261}'),
            "control pill must have moved off the tab row:\n{first_line}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib status_bar_row_is_always_present_and_holds_the_control_pill`
Expected: FAIL — the pill glyph `≡` is still on the first line (tab row), not the last.

- [ ] **Step 3: Make `status_h` always 1 and rename the area binding**

In `src/app/render/mod.rs`, replace:

```rust
        let tabs_h: u16 = 1;
        let spacer_h: u16 = 1;
        // seek = full-width seekbar row; title = now-playing row; controls = blank spacer below it. (status is unused.)
        let (seek_h, gap_h, title_h, controls_h, status_h): (u16, u16, u16, u16, u16) =
            if onerow || reserve_player_rows {
                (1, 0, 1, 1, 0)
            } else {
                (1, 0, 0, 0, 0)
            };
        let [tabs_area, _spacer_area, seek_area, _gap_area, title_area, _controls_area, _status_area, main_area] =
            Layout::vertical([
                Constraint::Length(tabs_h),
                Constraint::Length(spacer_h),
                Constraint::Length(seek_h),
                Constraint::Length(gap_h),
                Constraint::Length(title_h),
                Constraint::Length(controls_h),
                Constraint::Length(status_h),
                Constraint::Min(0),
            ])
            .areas(area);
```

with:

```rust
        let tabs_h: u16 = 1;
        let spacer_h: u16 = 1;
        let status_bar_h: u16 = 1;
        // seek = full-width seekbar row; title = now-playing row; controls = blank spacer below it.
        // status_bar is the persistent bottom row (control pill, session/queue state, toast) --
        // unlike the other player rows it is not conditional on onerow/reserve_player_rows.
        let (seek_h, gap_h, title_h, controls_h): (u16, u16, u16, u16) =
            if onerow || reserve_player_rows {
                (1, 0, 1, 1)
            } else {
                (1, 0, 0, 0)
            };
        let [tabs_area, _spacer_area, seek_area, _gap_area, title_area, _controls_area, status_bar_area, main_area] =
            Layout::vertical([
                Constraint::Length(tabs_h),
                Constraint::Length(spacer_h),
                Constraint::Length(seek_h),
                Constraint::Length(gap_h),
                Constraint::Length(title_h),
                Constraint::Length(controls_h),
                Constraint::Length(status_bar_h),
                Constraint::Min(0),
            ])
            .areas(area);
```

- [ ] **Step 4: Move the control pill call out of the tab-row block and into the new row**

In the same function, find the block:

```rust
        {
            // Control pill (m ⇌ ≡) on the far left of the tab bar.
            self.render_control_pill(f, tabs_area, &mut layout.playback);

            // Tabs occupy the space between the control pill (left) and VOL (right).
            let tabs_x = tabs_area.x + super::TABBAR_LEFT_RESERVE;
```

Delete the `self.render_control_pill(...)` line and its comment from inside that block (leave `tabs_x`/everything else in the block untouched — `TABBAR_LEFT_RESERVE` still reserves room for it visually consistent tab alignment, that constant is a separate concern from this task). Immediately after that whole tab-row block closes (right before the `let now_playing: Option<String> = ...` line), add:

```rust
        // Persistent bottom status bar: control pill lives here now instead of
        // the tab row. Session/queue segments and toast override land in
        // later tasks; for now this row renders only the pill.
        self.render_control_pill(f, status_bar_area, &mut layout.playback);
```

- [ ] **Step 5: Update `render_control_pill`'s doc comment**

It currently reads:

```rust
    /// Control pill on the far left of the tab bar: `  m ⇌ ≡  ` on an always-green
    /// background. Each icon is its assigned color when ON, or reverse-video
    /// (dark on green) when OFF. `m` mute and `⇌` remote are clickable.
    fn render_control_pill(&mut self, f: &mut Frame, tabs_area: Rect, layout: &mut LayoutPlayback) {
```

Change the first line only (the function still takes a generic `Rect` — it never assumed "tab row" internally, it only read `.x`/`.y`):

```rust
    /// Control pill on the far left of the status bar: `  m ⇌ ≡  ` on an always-green
```

Also rename the parameter from `tabs_area` to `area` for clarity since it's no longer the tab row:

```rust
    fn render_control_pill(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback) {
```

and update the two internal uses (`let (x, y) = (tabs_area.x, tabs_area.y);` → `let (x, y) = (area.x, area.y);`).

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --lib status_bar_row_is_always_present_and_holds_the_control_pill`
Expected: PASS

- [ ] **Step 7: Run the full test suite to check for collateral breakage**

Run: `cargo test --lib`
Expected: PASS. If any test asserts the pill glyphs (`m`, `⇌`, `≡`) appear on the *first* rendered line, or asserts exact row counts that assumed the old zero-height status row, it will need updating to match the new layout — that is expected fallout from this task, fix those assertions in place (don't skip/ignore them).

- [ ] **Step 8: Commit**

```bash
git add src/app/render/mod.rs src/app/mod.rs
git commit -m "Reclaim dormant status row as persistent bottom bar; relocate control pill"
```

---

### Task 2: Add the session/connection label next to the control pill

**Files:**
- Modify: `src/app/render/mod.rs` (new method `render_status_bar`, called from `App::render` in place of the bare `render_control_pill` call added in Task 1)
- Test: `src/app/mod.rs` `tests` module

**Interfaces:**
- Consumes: `self.remote_slot_state() -> RemoteSlotState` (`src/app/mod.rs`, variants `Off` / `AttachedSession` / `DirectRemote` / `LocalDaemon`), `self.stay_alive_ctrl: Option<stay_alive::StayAliveCtrl>` (`src/app/mod.rs`), `LayoutPlayback` (from Task 1).
- Produces: `fn render_status_bar(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback)` — replaces the direct `render_control_pill` call in `App::render`. Task 3 extends this same method with the right-aligned queue segment.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn status_bar_shows_direct_remote_label_next_to_pill() {
        let mut app = make_remote_app_stub(make_items(1), make_items(2));
        app.tab_idx = 0;
        app.set_queue_scope(QueueScope::Remote);

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains("REMOTE"),
            "expected a REMOTE label on the status bar for DirectRemote state:\n{last_line}"
        );
    }

    #[test]
    fn status_bar_has_no_session_label_when_remote_slot_is_off() {
        let mut app = make_app_stub();
        app.tab_idx = 0;

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            !last_line.contains("REMOTE") && !last_line.contains("ATTACHED") && !last_line.contains("DAEMON"),
            "expected no session label when nothing is connected:\n{last_line}"
        );
    }
```

Both tests reuse `make_remote_app_stub` — confirm it exists near `make_app_stub` in `src/app/mod.rs` before writing the test (it's used by the existing `power_queue_renders_scope_pills_and_hitboxes_for_direct_remote` test, so it does).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib status_bar_shows_direct_remote_label_next_to_pill status_bar_has_no_session_label_when_remote_slot_is_off`
Expected: FAIL — no `render_status_bar` exists yet, so `render_app_to_string` still only draws the bare pill from Task 1; neither label appears.

- [ ] **Step 3: Implement `render_status_bar` and wire it in**

In `src/app/render/mod.rs`, add a new method near `render_control_pill`:

```rust
    /// Persistent bottom status bar. Left side: control pill + a text label
    /// for session/connection state the pill's glyph alone doesn't spell out
    /// (RemoteSlotState, stay-alive). Right side (added in a later task):
    /// queue state, shown only on the Queue tab / Power View.
    fn render_status_bar(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback) {
        self.render_control_pill(f, area, layout);

        let remote_state = self.remote_slot_state();
        let mut spans: Vec<Span> = Vec::new();
        match remote_state {
            super::RemoteSlotState::Off => {}
            super::RemoteSlotState::AttachedSession => {
                spans.push(Span::styled(
                    " ATTACHED",
                    Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD),
                ));
            }
            super::RemoteSlotState::DirectRemote => {
                spans.push(Span::styled(
                    " REMOTE",
                    Style::default().fg(palette::PINE).add_modifier(Modifier::BOLD),
                ));
            }
            super::RemoteSlotState::LocalDaemon => {
                spans.push(Span::styled(
                    " DAEMON",
                    Style::default().fg(palette::PINE).add_modifier(Modifier::BOLD),
                ));
            }
        }
        if self.stay_alive_ctrl.is_some() {
            spans.push(Span::styled(" ALIVE", Style::default().fg(palette::FOAM)));
        }
        if !spans.is_empty() {
            let label_x = area.x + 9; // pill is always "  m ⇌ ≡  " = 9 cols wide
            let label_rect = Rect {
                x: label_x,
                y: area.y,
                width: area.width.saturating_sub(9),
                height: 1,
            };
            f.render_widget(Paragraph::new(Line::from(spans)), label_rect);
        }
    }
```

Then in `App::render`, replace the line added in Task 1:

```rust
        self.render_control_pill(f, status_bar_area, &mut layout.playback);
```

with:

```rust
        self.render_status_bar(f, status_bar_area, &mut layout.playback);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib status_bar_shows_direct_remote_label_next_to_pill status_bar_has_no_session_label_when_remote_slot_is_off`
Expected: PASS

- [ ] **Step 5: Run the full suite**

Run: `cargo test --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/app/render/mod.rs
git commit -m "Add session/connection label to the status bar"
```

---

### Task 3: Add the right-aligned queue-state segment (Queue tab / Power View only)

**Files:**
- Modify: `src/app/render/mod.rs` (`render_status_bar`, extended)
- Test: `src/app/mod.rs` `tests` module

**Interfaces:**
- Consumes: `self.tab_idx: usize`, `self.queue_dirty: bool`, `self.queue_source: crate::config::QueueSource`, `self.queue_is_saved_playlist() -> bool` (`src/app/actions.rs`), `self.visible_queue_scope() -> QueueScope` (`src/app/mod.rs`), `self.client.lock().unwrap().config.save_playlist_on_consume: bool` / `.save_playlist_on_consume_audio: bool` (established access pattern, see `src/app/actions.rs:4218` and `:4246`).
- Produces: extends `render_status_bar` in place; no new public interface for later tasks (Task 4 only touches the toast path, not this segment).

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn status_bar_shows_unsaved_marker_for_dirty_queue_on_queue_tab() {
        let mut app = make_app_stub();
        app.tab_idx = 1; // Queue tab
        app.queue_dirty = true;

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains("UNSAVED"),
            "expected an UNSAVED marker for a dirty queue on the Queue tab:\n{last_line}"
        );
    }

    #[test]
    fn status_bar_hides_queue_segment_outside_queue_and_power_view() {
        let mut app = make_app_stub();
        app.tab_idx = 0; // Home tab
        app.queue_dirty = true;

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            !last_line.contains("UNSAVED"),
            "queue state must not leak onto tabs where it isn't relevant:\n{last_line}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib status_bar_shows_unsaved_marker_for_dirty_queue_on_queue_tab status_bar_hides_queue_segment_outside_queue_and_power_view`
Expected: first test FAILs (no `UNSAVED` text exists yet); second PASSes trivially (nothing renders queue state anywhere yet) — that's fine, it becomes a meaningful regression guard once Step 3 lands.

- [ ] **Step 3: Extend `render_status_bar` with the right segment**

Append to the end of `render_status_bar` (after the left-segment `if !spans.is_empty() { ... }` block, still inside the function, before its closing `}`):

```rust
        if self.tab_idx == 1 {
            let mut right_spans: Vec<Span> = Vec::new();
            let source_label: Option<(String, Color)> = match &self.queue_source {
                crate::config::QueueSource::Playlist { name, .. } => {
                    Some((format!("PLAYLIST {name}"), palette::FOAM))
                }
                crate::config::QueueSource::Album => Some(("ALBUM".to_string(), palette::MUTED)),
                crate::config::QueueSource::Series => Some(("SERIES".to_string(), palette::MUTED)),
                crate::config::QueueSource::Shuffle => Some(("SHUFFLE".to_string(), palette::MUTED)),
                crate::config::QueueSource::Remote => Some(("REMOTE Q".to_string(), palette::MUTED)),
                crate::config::QueueSource::Collection { collection_type } => {
                    Some((collection_type.to_uppercase(), palette::MUTED))
                }
                crate::config::QueueSource::Unknown => None,
            };
            if let Some((label, color)) = source_label {
                right_spans.push(Span::styled(label, Style::default().fg(color)));
            }
            let autosave_on = self.queue_is_saved_playlist() && {
                let cfg = &self.client.lock().unwrap().config;
                cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
            };
            if autosave_on {
                right_spans.push(Span::styled(" AUTOSAVE", Style::default().fg(palette::PINE)));
            }
            if self.queue_dirty {
                right_spans.push(Span::styled(
                    " UNSAVED",
                    Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD),
                ));
            }
            if self.visible_queue_scope() == super::QueueScope::Remote {
                right_spans.push(Span::styled(" REMOTE QUEUE", Style::default().fg(palette::PINE)));
            }
            if !right_spans.is_empty() {
                let right_w: u16 = right_spans.iter().map(|s| s.content.width() as u16).sum();
                let left_end = area.x + 9; // pill width; label (if any) extends further but
                                            // right segment yielding to the pill alone is enough
                                            // to avoid the worst overlap on narrow terminals.
                let right_x = area.x + area.width.saturating_sub(right_w);
                if right_x >= left_end {
                    let right_rect = Rect {
                        x: right_x,
                        y: area.y,
                        width: right_w,
                        height: 1,
                    };
                    f.render_widget(Paragraph::new(Line::from(right_spans)), right_rect);
                }
                // else: terminal too narrow for both segments -- right segment drops
                // silently rather than overlapping the pill. (Design doc's open
                // question on narrow-terminal truncation: right segment yields first.)
            }
        }
```

This requires `Color` and `Widget::width` trait (`unicode_width::UnicodeWidthStr`, already imported in this file — confirmed by its existing use in `render()` for `vol_spans` width calculation) to be in scope; both already are, since `render_control_pill`/`render()` use `Color` and `.width()` the same way in this file.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib status_bar_shows_unsaved_marker_for_dirty_queue_on_queue_tab status_bar_hides_queue_segment_outside_queue_and_power_view`
Expected: PASS

- [ ] **Step 5: Run the full suite**

Run: `cargo test --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/app/render/mod.rs
git commit -m "Add right-aligned queue-state segment to the status bar"
```

---

### Task 4: Move the toast into the status bar and delete the old 3-row overlay

**Files:**
- Modify: `src/app/render/mod.rs` (`App::render` — delete the old toast block, add the new one; `toast_line` itself is unchanged)
- Test: `src/app/mod.rs` `tests` module

**Interfaces:**
- Consumes: `self.status: String`, `self.status_expires: Option<Instant>`, `self.system_notifications: bool`, `self.notif_failed: bool` (all pre-existing `App` fields), `Self::toast_line(&str) -> Line<'static>` (pre-existing, unchanged).
- Produces: nothing new for later tasks — this is the last task in the plan.

Today's toast block, verified in `App::render` (right after the tab-idx branch that picks which panel to draw into `main_area`):

```rust
        if !self.status.is_empty() && (!self.system_notifications || self.notif_failed) {
            let toast_rect = Rect {
                x: area.x,
                y: area.y + area.height - 3,
                width: area.width,
                height: 3,
            };
            f.render_widget(Clear, toast_rect);
            f.render_widget(
                Paragraph::new(Self::toast_line(&self.status))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(palette::TEXT).bg(palette::IRIS))
                    .block(
                        Block::default()
                            .style(Style::default().fg(palette::TEXT).bg(palette::IRIS))
                            .padding(ratatui::widgets::Padding::vertical(1)),
                    ),
                toast_rect,
            );
        }
```

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn toast_renders_in_status_bar_without_covering_main_content_above_it() {
        let mut app = make_app_stub();
        app.tab_idx = 0;
        app.status = "Saved [Y]".to_string();
        app.status_expires = Some(std::time::Instant::now() + Duration::from_secs(5));

        let rendered = render_app_to_string(&mut app, 80, 24);
        let lines: Vec<&str> = rendered.lines().collect();
        let last_line = lines.last().unwrap();

        assert!(
            last_line.contains("Saved"),
            "expected the toast text on the final row:\n{last_line}"
        );
        // Old behavior covered 3 rows with Clear+overlay; new behavior must
        // only touch the single bottom row, leaving the row above untouched.
        let second_to_last = lines[lines.len() - 2];
        assert!(
            !second_to_last.contains("Saved"),
            "toast must not spill onto the row above the status bar:\n{second_to_last}"
        );
    }

    #[test]
    fn status_bar_shows_normal_content_when_no_toast_is_active() {
        let mut app = make_app_stub();
        app.tab_idx = 0;
        app.status = String::new();

        let rendered = render_app_to_string(&mut app, 80, 24);
        let last_line = rendered.lines().last().unwrap();

        assert!(
            last_line.contains('\u{2261}'),
            "expected the control pill to still render when no toast is active:\n{last_line}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib toast_renders_in_status_bar_without_covering_main_content_above_it status_bar_shows_normal_content_when_no_toast_is_active`
Expected: `toast_renders_in_status_bar_...` FAILs (toast still draws as the old 3-row overlay at `area.height - 3`, not the final row via the new bar); `status_bar_shows_normal_content_...` PASSes already (Task 2/3 cover this) — keep it as a regression guard for Step 3.

- [ ] **Step 3: Replace the toast block and route it through `status_bar_area`**

Delete the old toast block shown above from `App::render`.

Change the call added in Task 2:

```rust
        self.render_status_bar(f, status_bar_area, &mut layout.playback);
```

to:

```rust
        let show_toast = !self.status.is_empty() && (!self.system_notifications || self.notif_failed);
        if show_toast {
            f.render_widget(
                Paragraph::new(Self::toast_line(&self.status))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(palette::TEXT).bg(palette::IRIS)),
                status_bar_area,
            );
        } else {
            self.render_status_bar(f, status_bar_area, &mut layout.playback);
        }
```

Note this drops the old `Clear` + `Block`-with-vertical-padding: `Clear` is no longer needed because `status_bar_area` is a reserved row nothing else draws into (there's nothing underneath left to clear), and the vertical padding doesn't apply to a 1-row area. `Alignment` and `Block`/`Clear` imports may now be partially unused elsewhere in the file — check with Step 5 before removing any `use` lines (don't remove `Alignment` if `render_title_row` or another method still uses it).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib toast_renders_in_status_bar_without_covering_main_content_above_it status_bar_shows_normal_content_when_no_toast_is_active`
Expected: PASS

- [ ] **Step 5: Run the full suite and check for now-unused imports**

Run: `cargo build 2>&1 | grep -i "unused import"`
Expected: no output. If `Clear` or `Block` or `ratatui::widgets::Padding` show up as unused (they were only used in the deleted block), remove exactly those now-dead `use` lines — nothing else.

Run: `cargo test --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/app/render/mod.rs
git commit -m "Move toast rendering into the persistent status bar; delete the 3-row overlay"
```

---

## Plan Self-Review

- **Spec coverage:** Layout placement (Task 1) ✓. Control pill relocation (Task 1) ✓. Session/connection label (Task 2) ✓. Queue-state right segment, gated to Queue/Power View (Task 3) ✓. Toast full-width takeover replacing the 3-row overlay (Task 4) ✓. Res/audio/sub chips and `VOL` badge explicitly left untouched (never referenced by any task) ✓. Narrow-terminal truncation open question resolved in Task 3 Step 3 (right segment drops before overlapping the pill).
- **Placeholder scan:** No TBD/TODO; every step shows literal code, not a description of code.
- **Type consistency:** `render_status_bar(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutPlayback)` signature introduced in Task 2 is reused unchanged in Task 3 (extended in place) and called unchanged (inside the `else` branch) in Task 4. `RemoteSlotState`/`QueueScope` referenced via `super::` consistently, matching the existing `render_control_pill` pattern.
