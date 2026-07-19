# Music Library Group Pills Row Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the music library group pills from the power-view title/header row onto their own row directly below the title, matching other grouped library layouts.

**Architecture:** Keep the existing `render_power_music_group_pills_row` control renderer and move only its call site. The full-width power-view header should render the music library title marker only; the pills should render in the right library column immediately below that header, and the music group album list should start one row below the pills.

**Tech Stack:** Rust, Ratatui, existing `App` render methods, existing `LayoutPower` selector hitboxes, `cargo test`.

## Global Constraints

- Issue: #261, "Move group pills on music library back to a row below the panel title".
- The panel title line should contain only the title.
- The music library group pills should appear on a separate row directly below the title.
- Existing pill behavior is unchanged: selection, grouping behavior, focus/navigation, and scrolling continue to work as before.
- The change is surgical: move/reposition the existing pill row rather than redesigning the group controls or refactoring panel headers broadly.
- Do not change music library grouping behavior.
- Do not change pill selection semantics, labels, ordering, or scroll mechanics.
- Do not redesign shared panel headers.
- Work in an isolated worktree branch created from `origin/main`; do not commit on `main`.
- Do not open a pull request until implementation and verification are complete.

---

## File Structure

- Modify `src/app/render/power/mod.rs`: move the music-group pills call out of the top header row, render the title marker alone on the header row, reserve a one-row pill band in the right library column for music-group libraries, and update existing render tests that currently assert the inline-row behavior.
- Modify `src/app/render/power/music.rs`: update comments that currently describe music group pills as top-rule-row controls; keep the renderer implementation unchanged unless the tests reveal a required coordinate adjustment.
- No production files should be created.
- No docs or ADR changes are expected for this UI-only layout adjustment.

---

### Task 1: Move Music Group Pills Below The Title

**Files:**
- Modify: `src/app/render/power/mod.rs`
- Modify: `src/app/render/power/music.rs`
- Test: `src/app/render/power/mod.rs`

**Interfaces:**
- Consumes: `App::render_power_music_group_pills_row(&mut self, f: &mut Frame, row_area: Rect, lib_idx: usize, layout: &mut LayoutPower)` from `src/app/render/power/music.rs`.
- Produces: unchanged `layout.selector_tabs: Vec<(Rect, usize)>`, with music group pill hitboxes registered on the new pills row instead of the top header row.

- [ ] **Step 1: Update the existing tests to describe the desired layout**

In `src/app/render/power/mod.rs`, replace the test named `music_group_pills_and_marker_render_on_top_rule_row` with this test:

```rust
    #[test]
    fn music_group_pills_render_on_row_below_title_marker() {
        let mut app = make_power_music_group_app();
        app.power_left_width = 20;
        let width = 100u16;
        let height = 20u16;
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_view(f, Rect::new(0, 0, width, height), &mut layout);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let row0 = out.lines().next().unwrap();
        let row1 = out.lines().nth(1).unwrap();

        assert!(
            !row0.contains("Alpha") && !row0.contains("Beta"),
            "expected group pills to move off the title row:\n{out}"
        );
        assert!(
            row1.contains("Alpha") && row1.contains("Beta"),
            "expected group pills on the row below the title:\n{out}"
        );

        let rchar_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.rfind(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };
        let char_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.find(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };

        let music_x = rchar_x(row0, "Music");
        assert!(
            music_x + "Music".len() as u16 + 1 >= width,
            "expected the Music title marker pinned to the far right of the title row:\n{out}"
        );

        let buf = term.backend().buffer();
        assert_eq!(
            buf[(music_x, 0)].bg,
            palette::FOAM,
            "expected the Music marker to keep the standard blue pill background"
        );
        assert_eq!(
            buf[(music_x, 0)].fg,
            palette::BASE,
            "expected the Music marker to keep the standard base (black) text"
        );

        let right_col_x = app.power_left_width + 1;
        assert!(
            row0.chars()
                .take(right_col_x as usize)
                .all(|c| c == '\u{2501}'),
            "expected a plain dash rule over the left column on the title row:\n{out}"
        );
        assert!(
            row1.chars()
                .take(right_col_x as usize)
                .all(|c| c == ' '),
            "expected the pill row to be confined to the right library column:\n{out}"
        );

        let alpha_x = char_x(row1, "Alpha");
        assert!(
            alpha_x >= right_col_x,
            "expected pills confined to the right column"
        );
        assert_eq!(buf[(alpha_x, 1)].bg, palette::FOAM);
        assert_eq!(
            buf[(alpha_x, 1)].fg,
            palette::YELLOW,
            "expected the selected group pill to use yellow text"
        );
        let beta_x = char_x(row1, "Beta");
        assert_eq!(buf[(beta_x, 1)].bg, palette::FOAM);
        assert_eq!(
            buf[(beta_x, 1)].fg,
            palette::BASE,
            "expected a non-selected group pill to stay blue with base text"
        );

        let (gap_start, gap_end) = (alpha_x.min(beta_x), alpha_x.max(beta_x));
        let between: String = row1
            .chars()
            .skip(gap_start as usize)
            .take((gap_end - gap_start) as usize)
            .collect();
        assert!(
            between.contains('\u{2501}'),
            "expected a dash rule between adjacent pills, not blank space:\n{between:?}"
        );

        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 1, "expected selector hitboxes on the pills row");
            assert!(
                rect.x >= right_col_x,
                "expected selector hitboxes confined to the right column"
            );
        }

        let row2 = out.lines().nth(2).unwrap();
        assert!(
            row2.contains("Alpha") || row2.contains("First Album"),
            "expected album list content to start below the separate pill row:\n{out}"
        );
    }
```

In the same file, update `music_group_pills_scroll_within_reserved_space_when_overflowing` so it reads `row0` for the title marker and `row1` for the pills:

```rust
        let row0 = out.lines().next().unwrap();
        let row1 = out.lines().nth(1).unwrap();

        assert!(
            row1.contains('\u{203a}'),
            "expected a right scroll indicator on the pills row:\n{out}"
        );
        assert!(
            row0.contains("Music"),
            "expected the Music marker to keep rendering on the title row when pills overflow:\n{out}"
        );

        let rchar_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.rfind(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };

        let music_x = rchar_x(row0, "Music");
        assert!(
            music_x + "Music".len() as u16 + 1 >= width,
            "expected the Music marker to remain pinned to the far right:\n{out}"
        );
        let right_indicator_x = rchar_x(row1, "\u{203a}");
        assert!(
            right_indicator_x < width,
            "expected the right scroll indicator to stay inside the pill row:\n{out}"
        );

        let right_col_x = (app.power_left_width + 1) as usize;
        assert!(
            row0.chars().take(right_col_x).all(|c| c == '\u{2501}'),
            "expected a plain dash rule over the left column on the title row:\n{out}"
        );
        assert!(
            row1.chars().take(right_col_x).all(|c| c == ' '),
            "expected the pill row to be confined to the right library column:\n{out}"
        );

        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 1, "expected pill hitboxes on the pills row");
            assert!(
                rect.x as usize >= right_col_x,
                "expected pill hitboxes confined to the right column"
            );
            assert!(
                rect.x + rect.width <= width,
                "expected pill hitboxes confined to the visible pill row"
            );
        }
```

- [ ] **Step 2: Run the focused tests and verify they fail for the old layout**

Run:

```bash
cargo test -p mbv --lib render::power::tests::music_group_pills -- --nocapture
```

Expected: FAIL. The old implementation renders `Alpha` and `Beta` on row 0, so the new `music_group_pills_render_on_row_below_title_marker` assertion that row 0 has no group pills should fail.

- [ ] **Step 3: Render only the title marker on the header row**

In `src/app/render/power/mod.rs`, inside `App::render_power_view`, replace the `else if is_music_group_lib` branch so it no longer calls `render_power_music_group_pills_row` on `crumb_row`. The branch should render the full-width rule and the right-aligned library title marker only:

```rust
            } else if is_music_group_lib {
                layout.breadcrumbs = Vec::new();
                layout.selector_tabs = Vec::new();
                let lib_idx = self.power_left_tab - 1;

                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "\u{2501}".repeat(area.width as usize),
                        Style::default().fg(palette::FOAM),
                    ))),
                    Rect {
                        x: area.x,
                        y: crumb_row,
                        width: area.width,
                        height: 1,
                    },
                );

                let right_col_x = area.x + left_w + 1;
                let right_col_w = right_w.saturating_sub(1);
                let marker_text = format!(" {} ", self.libs[lib_idx].library.name);
                let marker_w = (marker_text.width() as u16).min(right_col_w);

                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        marker_text,
                        Style::default().fg(palette::BASE).bg(palette::FOAM),
                    ))),
                    Rect {
                        x: right_col_x + right_col_w.saturating_sub(marker_w),
                        y: crumb_row,
                        width: marker_w,
                        height: 1,
                    },
                );
```

- [ ] **Step 4: Render the pills on a separate right-column row and shift only the music library list down**

In `src/app/render/power/mod.rs`, after the `let (lib_area, queue_area) = ...;` block and before `self.render_power_queue(...)`, create a `render_lib_area` variable. For music-group libraries, render the pill row at the top of `lib_area` and pass a one-row-shorter area into `render_power_library`:

```rust
        let mut render_lib_area = lib_area;
        if self.power_left_tab > 0 && self.is_music_group_view(self.power_left_tab - 1) {
            let lib_idx = self.power_left_tab - 1;
            if lib_area.height > 0 {
                let pills_area = Rect {
                    x: lib_area.x,
                    y: lib_area.y,
                    width: lib_area.width,
                    height: 1,
                };
                self.render_power_music_group_pills_row(f, pills_area, lib_idx, layout);
                render_lib_area = Rect {
                    y: lib_area.y + 1,
                    height: lib_area.height.saturating_sub(1),
                    ..lib_area
                };
            } else {
                layout.selector_tabs = Vec::new();
            }
        }

        self.render_power_queue(f, queue_area, queue_focused, layout);
        self.render_power_library(f, render_lib_area, left_focused, layout);
```

Remove the old direct call:

```rust
        self.render_power_library(f, lib_area, left_focused, layout);
```

- [ ] **Step 5: Update stale comments**

In `src/app/render/power/mod.rs`, replace the comment above the music-group header branch:

```rust
        // Music-group libraries use the standard right-aligned library title
        // marker on the header row. Their group selector pills render on their
        // own row at the top of the right library column below this title row.
```

In `src/app/render/power/music.rs`, replace the `render_power_music_group_view` doc comment paragraph that says the group selector is rendered on the top rule row:

```rust
    /// Renders the grouped-by-artist album list for a music group library. The
    /// group-selector pills for this view are rendered by the caller on their
    /// own row above this list (`render_power_music_group_pills_row`) -- this
    /// method starts directly with the album list.
```

- [ ] **Step 6: Run the focused tests and verify they pass**

Run:

```bash
cargo test -p mbv --lib render::power::tests::music_group_pills -- --nocapture
```

Expected: PASS. Both music group pill tests pass, and their output should show the `Music` marker on row 0 with group pills and selector hitboxes on row 1.

- [ ] **Step 7: Run formatting and the relevant broader render test module**

Run:

```bash
cargo fmt --all -- --check
cargo test -p mbv --lib render::power -- --nocapture
```

Expected: both commands PASS.

- [ ] **Step 8: Run pre-PR quality checks**

Run:

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: both commands PASS.

- [ ] **Step 9: Commit and push the branch**

Run:

```bash
git status --short
git add src/app/render/power/mod.rs src/app/render/power/music.rs
git commit -m "fix: move music group pills below title"
git push -u origin HEAD
```

Expected: the commit includes only the two render files, and the branch is pushed to `origin`.

- [ ] **Step 10: Open the PR**

Run:

```bash
gh pr create --repo slatkin/mbv --base main --fill
```

Expected: GitHub returns a PR URL. The PR should reference `Fixes #261` in the body if `--fill` does not include it automatically.

---

## Self-Review

- Spec coverage: The plan moves the pills to a row below the title, leaves the title row as title-only, preserves pill renderer behavior, verifies hitbox coordinates, and keeps scope to the existing render files.
- Placeholder scan: No placeholders, TBDs, or unspecified test steps remain.
- Type consistency: The plan uses the existing `render_power_music_group_pills_row` signature and existing `LayoutPower::selector_tabs` type unchanged.
