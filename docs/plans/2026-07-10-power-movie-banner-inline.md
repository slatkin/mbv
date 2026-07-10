# Power View Movie Banner: Inline Placement + Left-Panel Backdrop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers-optimized:subagent-driven-development (recommended) or superpowers-optimized:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the Power View compact movie banner from a fixed top-pinned slot to an inline position directly under the selected movie's row (scrolling with the list), and make the left card panel show the selected movie's poster/backdrop (falling back to queue art if the movie has no images) whenever the banner is showing.

**Architecture:** `render_power_list` (src/app/render/power/list.rs) already tracks scroll offset against a *display-row* sequence (not raw item indices) for its letter-grouped and artist-grouped branches — it interleaves `Spacer`/`LetterHeader` pseudo-rows and reclamps scroll against display-row position each frame. This plan extends that same mechanism: a movie's row gets `COMPACT_BANNER_TOTAL_ROWS` blank filler rows inserted immediately after it in the display-row sequence (plain branch and letter-grouped branch both), so the list's own scroll math naturally keeps the selected row + banner on screen. After the `List` widget renders, the banner's actual content (image + text, which `List`/`ListItem` cannot hold) is drawn separately into the filler gap's computed `Rect`, exactly as `render_power_compact_detail` already draws today — just repositioned per-frame instead of pinned to the top. `render_power_library`'s current `show_compact_movie_detail` special-case (fixed top/banner/list split + a cursor-nudge scroll hack) is deleted entirely; the plain `render_power_list` call already used for ordinary library types now handles movies correctly without special-casing. Separately, `render_power_card`'s existing `power_detail_pinned` image branch (Backdrop→Primary→Logo priority, focus-gated) is widened to also fire when the compact banner is active (not just when expanded detail is pinned), with a new fallback to queue-cursor art if the movie has no images at all — scoped to the compact case only.

**Tech Stack:** Rust, ratatui (TUI), existing `mbv` app crate conventions (Serena-indexed, GitNexus-tracked).

**Assumptions:**
- Assumes the two spec-change comments on issue #114 (left-panel backdrop, inline banner placement) are the only behavior changes in scope — will NOT touch Alt+M/Enter input handling, expanded-detail rendering, or the 12-row compact poster height (confirmed correct, not a bug).
- Assumes `power_selected_movie_item(lib_idx)` remains the single source of truth for "is there a leaf movie selected right now" (covers both search results and nav_stack) — will NOT work correctly if a future change makes movie selection determinable some other way without updating this helper.
- Assumes `COMPACT_DETAIL_H = 13` (banner content rows) and the 1-row gap below it stay the same visual sizes, just repositioned — will NOT change how much content fits in the banner.

---

## File Structure

- **Modify `src/app/render/power/card.rs`** (`render_power_card`): widen the existing `power_detail_pinned` branch's trigger to also cover the compact-banner case, with a queue-art fallback scoped to that case.
- **Modify `src/app/render/power/list.rs`** (`render_power_list`): add banner-row-reservation consts/helper; interleave filler rows in both the plain and letter-grouped display-row sequences; draw the banner into the reserved gap after the list renders; extend the scroll clamp to keep the full banner visible.
- **Modify `src/app/render/power/mod.rs`** (`render_power_library`): delete the `show_compact_movie_detail` special-case branch and its local consts — the final `else { render_power_list }` arm now handles movies correctly on its own.
- **Modify `src/app/input.rs`**: fix the left-panel search-result click handler to consult `left_row_map` when populated (mirrors the existing feed-group branch), since filler rows can now appear in movie-library search results too.

---

## Task 1: Left card panel shows the selected movie (with queue-art fallback) during the compact banner

**Files:**
- Modify: `src/app/render/power/card.rs`
- Test: `src/app/render/power/card.rs` (`tests` module)

**Security flag:** `none`

**Does NOT cover:** Expanded-detail-pinned's no-image behavior (stays blank-on-no-image, unchanged) — only the new compact-banner branch gets the queue-art fallback. Does NOT cover non-movie library types (album/series/home-video branches unchanged, evaluated before this new branch).

- [ ] **Step 1: Write failing test for the new branch and its fallback**

Add to the `tests` module in `src/app/render/power/card.rs` (mirror the existing test setup style used for `power_detail_pinned` tests in this file — a movies-library `App` stub with one leaf movie selected, `power_detail_item: None`):

```rust
#[test]
fn compact_banner_active_extends_pinned_image_branch_with_queue_fallback() {
    // A leaf movie is selected (banner would show) but detail is NOT pinned.
    let mut app = make_power_card_movie_app(); // to be added alongside existing card-test helpers
    app.power_focus = PowerFocus::Left;

    // Movie has no card image at all: card_image_states holds Some(None) once the
    // (simulated) fetch completes with nothing found.
    let cache_key = "movie-focused:P".to_string();
    app.card_image_states.insert(cache_key, None);

    // Queue has an item, so the fallback should render that item's art instead of
    // a blank card.
    let (rows_used, loading) = render_card_for_test(&mut app); // helper wrapping render_power_card via TestBackend, added in this task
    assert!(!loading);
    assert!(rows_used > 0, "expected fallback queue-art rows, got {rows_used}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib compact_banner_active_extends_pinned_image_branch_with_queue_fallback`
Expected: FAIL — either compile error (missing test helpers) or the new branch not yet present, so the movie's blank state renders `rows_used == 0` instead of falling back to queue art.

- [ ] **Step 3: Implement the branch extension**

In `src/app/render/power/card.rs`, change the trigger guard from `power_detail_pinned` alone to also cover the compact-banner case, and add the fallback. Replace:

```rust
        let lib_focused = matches!(self.power_focus, PowerFocus::Left);
        let power_detail_pinned = lib_focused
            && self.power_left_tab > 0
            && self.libs[self.power_left_tab - 1]
                .power_detail_item
                .is_some();
        if power_detail_pinned {
            // (handled below)
        } else if lib_focused
```

with:

```rust
        let lib_focused = matches!(self.power_focus, PowerFocus::Left);
        let power_detail_pinned = lib_focused
            && self.power_left_tab > 0
            && self.libs[self.power_left_tab - 1]
                .power_detail_item
                .is_some();
        // Compact banner active: a leaf movie is selected but expanded detail is
        // not pinned. Mirrors power_detail_pinned's image priority below, but
        // (unlike power_detail_pinned) falls back to queue-cursor art if the
        // movie has no card image at all instead of showing a blank card.
        let compact_banner_active = lib_focused
            && !power_detail_pinned
            && self.power_left_tab > 0
            && self
                .power_selected_movie_item(self.power_left_tab - 1)
                .is_some();
        if power_detail_pinned || compact_banner_active {
            // (handled below)
        } else if lib_focused
```

Then, right before the existing `if power_detail_pinned { ... return self.render_card_image(...); }` block, insert the new branch's handling. Replace:

```rust
        if power_detail_pinned {
            let (detail_id, series_id) = {
                let lib_idx = self.power_left_tab - 1;
                let d = self.libs[lib_idx].power_detail_item.as_ref().unwrap();
                (d.id.clone(), d.series_id.clone())
            };
            let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
            let cache_key = format!("{}:P", detail_id);
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), detail_id, series_id, img_types);
            }
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        let (cursor, items) = {
```

with:

```rust
        if power_detail_pinned {
            let (detail_id, series_id) = {
                let lib_idx = self.power_left_tab - 1;
                let d = self.libs[lib_idx].power_detail_item.as_ref().unwrap();
                (d.id.clone(), d.series_id.clone())
            };
            let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
            let cache_key = format!("{}:P", detail_id);
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), detail_id, series_id, img_types);
            }
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        if compact_banner_active {
            let lib_idx = self.power_left_tab - 1;
            let item = self.power_selected_movie_item(lib_idx).unwrap();
            let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
            let cache_key = format!("{}:P", item.id);
            if self.images_enabled() {
                self.fetch_card_image(
                    cache_key.clone(),
                    item.id.clone(),
                    item.series_id.clone(),
                    img_types,
                );
            }
            // Some(None) means the fetch completed and found no Backdrop/Primary/
            // Logo at all — fall through to the queue-cursor default below instead
            // of showing a blank card. Absent-from-map (still loading) still
            // renders via render_card_image so the blank/loading placeholder shows
            // correctly while the fetch is in flight.
            let has_no_image = matches!(self.card_image_states.get(&cache_key), Some(None));
            if !has_no_image {
                return self.render_card_image(f, area, &cache_key, area.height.min(18));
            }
        }

        let (cursor, items) = {
```

- [ ] **Step 4: Add test helpers used in Step 1**

In the `tests` module of `src/app/render/power/card.rs`, add (next to existing helpers in this file):

```rust
fn make_power_card_movie_app() -> App {
    let mut app = make_app_stub();
    app.power_left_tab = 1;

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut focused = make_item("Focused Movie", "Movie");
    focused.id = "movie-focused".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![focused],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
        }],
        search: None,
        feed_home_video: None,
        power_detail_item: None,
        power_detail_scroll: 0,
    });

    // Queue has one item so the fallback has art to show.
    let mut track = make_item("Queued Track", "Audio");
    track.id = "queued-1".into();
    app.player_tab.items = vec![track];
    app.player_tab.queue_cursor = 0;

    app
}

fn render_card_for_test(app: &mut App) -> (u16, bool) {
    let backend = TestBackend::new(40, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut result = (0, false);
    term.draw(|f| {
        result = app.render_power_card(f, Rect::new(0, 0, 40, 18));
    })
    .unwrap();
    result
}
```

Adjust `make_item`/`LibraryTab`/`BrowseLevel`/`make_app_stub` field names to match whatever this file's existing tests already use (check the existing `power_detail_pinned` tests in this same `tests` module for the exact call signatures before finalizing — do not guess field names not already present in this file).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib compact_banner_active_extends_pinned_image_branch_with_queue_fallback`
Expected: PASS

- [ ] **Step 6: Run full card.rs test module to check no regressions**

Run: `cargo test --lib render::power::card::tests`
Expected: PASS (all tests, including pre-existing `power_detail_pinned` and album/series/home-video branch tests)

- [ ] **Step 7: Commit**

```bash
git add src/app/render/power/card.rs
git commit -m "Show selected movie's backdrop/poster in the left panel during the compact banner, falling back to queue art when the movie has no images"
```

---

## Task 2: Add banner-row reservation consts and helper to render_power_list

**Files:**
- Modify: `src/app/render/power/list.rs`

**Security flag:** `none`

**Does NOT cover:** Actually inserting the filler rows into the display-row sequences (Task 3 for the plain branch, Task 4 for the letter-grouped branch) — this task only adds the shared building blocks they both call.

- [ ] **Step 1: Add the consts and helper method**

In `src/app/render/power/list.rs`, insert before the `impl App` block containing `render_power_list`:

```rust
/// Rows the compact movie banner occupies inline in the library list: the
/// banner's own content (title/meta/overview/poster, rendered by
/// `render_power_compact_detail`) plus a 1-row blank separator before the
/// next list row — matching the spacing the banner already used when it was
/// pinned to the top of the panel.
const COMPACT_BANNER_CONTENT_ROWS: usize = 13;
const COMPACT_BANNER_GAP_ROWS: usize = 1;
const COMPACT_BANNER_TOTAL_ROWS: usize = COMPACT_BANNER_CONTENT_ROWS + COMPACT_BANNER_GAP_ROWS;
```

Then, inside the existing `impl App` block in this file (the one that currently holds only `render_power_list`), add a new method:

```rust
    /// Filler-row count to reserve immediately after the selected movie's row
    /// in `lib_idx`'s display-row sequence: `COMPACT_BANNER_TOTAL_ROWS` when a
    /// leaf movie is selected and expanded detail is not open, else 0 (no
    /// banner — ordinary list rendering, unchanged from before this feature).
    fn compact_banner_rows(&self, lib_idx: usize) -> usize {
        if self.libs[lib_idx].power_detail_item.is_some() {
            return 0;
        }
        if self.power_selected_movie_item(lib_idx).is_some() {
            COMPACT_BANNER_TOTAL_ROWS
        } else {
            0
        }
    }
```

- [ ] **Step 2: Wire banner_rows into render_power_list's top-level state**

In `render_power_list`, right after the existing block that computes `(items, cursor, stored_scroll, total_count)` (the `if self.power_left_tab == 0 { ... } else { ... }` assignment), add:

```rust
        // Reserved filler-row count for the compact movie banner, 0 for every
        // library type/state except "leaf movie selected, detail not pinned".
        let banner_rows: usize = if self.power_left_tab > 0 {
            self.compact_banner_rows(self.power_left_tab - 1)
        } else {
            0
        };
```

- [ ] **Step 3: Run existing list tests to confirm no behavior change yet**

Run: `cargo test --lib render::power::list::tests`
Expected: PASS — `banner_rows` is computed but not yet consumed anywhere, so no rendering changes.

- [ ] **Step 4: Commit**

```bash
git add src/app/render/power/list.rs
git commit -m "Add compact-banner filler-row consts and detection helper to render_power_list"
```

---

## Task 3: Interleave banner filler rows in the plain (non-grouped, non-lettered) list branch

**Files:**
- Modify: `src/app/render/power/list.rs`
- Test: `src/app/render/power/list.rs` (`tests` module)

**Security flag:** `none`

**Does NOT cover:** The letter-grouped branch (Task 4) — this task only changes the final `else` branch, which is what a movies library with fewer than 50 items uses.

- [ ] **Step 1: Write failing test — banner follows cursor to a non-zero row**

Add to `src/app/render/power/list.rs`'s `tests` module (reuse this file's existing `render_power_list`-to-string test helper and movie-library fixture pattern — check the existing letter-group tests in this file for the exact helper names before writing this):

```rust
#[test]
fn compact_banner_appears_under_selected_row_not_pinned_to_top() {
    let mut app = make_power_movie_list_app(vec!["First", "Second Selected", "Third"]);
    // Select the second item (index 1) — banner must render after ITS row, not row 0.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;

    let out = render_power_list_to_string(&mut app, 40, 20);
    let lines: Vec<&str> = out.lines().collect();

    assert!(lines[0].contains("First"), "row above cursor unaffected:\n{out}");
    assert!(
        lines[1].contains("Second Selected"),
        "selected row itself, unaffected by banner:\n{out}"
    );
    assert!(
        out.contains("compact movie banner"),
        "banner content expected somewhere after the selected row:\n{out}"
    );
    // Third item pushed down by the 14 reserved banner rows (13 content + 1 gap).
    assert!(
        lines[16].contains("Third"),
        "row below the selected item pushed down past the banner:\n{out}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib compact_banner_appears_under_selected_row_not_pinned_to_top`
Expected: FAIL — banner filler rows not yet inserted, so "Third" appears immediately at line 2, not line 16.

- [ ] **Step 3: Implement the plain-branch change**

Replace the final `else` branch's body in `render_power_list` (`src/app/render/power/list.rs`). The current body is:

```rust
        } else {
            let offset =
                stored_scroll.clamp(cursor.saturating_sub(visible.saturating_sub(1)), cursor);
            final_offset = offset;

            let list_items: Vec<ListItem> = items
                .iter()
                .skip(offset)
                .take(visible)
                .enumerate()
                .map(|(i, item)| {
                    let abs = offset + i;
                    let selected = abs == cursor;

                    // Compute name and duration as separate strings so they can be styled
                    // independently: name in the normal fg, duration in OVERLAY (no parens).
                    let (item_name, dur_str) = if item.is_folder {
                        let name = if item.item_type == "Folder" && item.total_count > 0 {
                            format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                        } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
                            format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                        } else {
                            item.display_name()
                        };
                        (name, String::new())
                    } else {
                        let dur = if item.runtime_ticks > 0 {
                            format!(
                                " {}",
                                fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                            )
                        } else {
                            String::new()
                        };
                        (item.display_name(), dur)
                    };

                    let avail = (area.width as usize).saturating_sub(2);
                    let name_w = avail.saturating_sub(dur_str.width());
                    let title = trunc_str(&item_name, name_w);
                    let fg = if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };

                    let mut spans: Vec<Span> = if selected && focused {
                        vec![
                            Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                            Span::styled(
                                title,
                                Style::default()
                                    .fg(palette::IRIS)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]
                    } else {
                        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                    };
                    if !dur_str.is_empty() {
                        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let mut state = ListState::default();
            state.select(Some(cursor.saturating_sub(offset)));
            layout.cursor_screen_y = Some(content_area.y + (cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            if focused && n > visible {
                let max_off = n.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None)
                        .end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area,
                    &mut sb,
                );
            }
        }
```

Replace it with:

```rust
        } else {
            enum DisplayRow {
                Item(usize),
                BannerFiller,
            }

            let mut display_rows: Vec<DisplayRow> = Vec::with_capacity(n + banner_rows);
            for i in 0..n {
                display_rows.push(DisplayRow::Item(i));
                if banner_rows > 0 && i == cursor {
                    for _ in 0..banner_rows {
                        display_rows.push(DisplayRow::BannerFiller);
                    }
                }
            }
            let total_display = display_rows.len();
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
                .unwrap_or(0);

            // Lower bound normally just keeps the cursor row visible; when a
            // banner follows it, extend the lower bound so scrolling keeps
            // pulling up until the whole banner is visible too (clamped to
            // display_cursor itself if the viewport could never fit both).
            let lower_bound = (display_cursor + banner_rows)
                .saturating_sub(visible.saturating_sub(1))
                .min(display_cursor);
            let offset = stored_scroll.clamp(lower_bound, display_cursor);
            final_offset = offset;

            let list_items: Vec<ListItem> = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|row| match row {
                    DisplayRow::BannerFiller => ListItem::new(Line::default()),
                    DisplayRow::Item(idx) => {
                        let item = &items[*idx];
                        let selected = *idx == cursor;

                        // Compute name and duration as separate strings so they can be styled
                        // independently: name in the normal fg, duration in OVERLAY (no parens).
                        let (item_name, dur_str) = if item.is_folder {
                            let name = if item.item_type == "Folder" && item.total_count > 0 {
                                format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                            } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
                                format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                            } else {
                                item.display_name()
                            };
                            (name, String::new())
                        } else {
                            let dur = if item.runtime_ticks > 0 {
                                format!(
                                    " {}",
                                    fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                                )
                            } else {
                                String::new()
                            };
                            (item.display_name(), dur)
                        };

                        let avail = (area.width as usize).saturating_sub(2);
                        let name_w = avail.saturating_sub(dur_str.width());
                        let title = trunc_str(&item_name, name_w);
                        let fg = if focused {
                            palette::WHITE
                        } else {
                            palette::SUBTLE
                        };

                        let mut spans: Vec<Span> = if selected && focused {
                            vec![
                                Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                                Span::styled(
                                    title,
                                    Style::default()
                                        .fg(palette::IRIS)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ]
                        } else {
                            vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                        };
                        if !dur_str.is_empty() {
                            spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                        }
                        ListItem::new(Line::from(spans))
                    }
                })
                .collect();

            layout.left_row_map = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|row| match row {
                    DisplayRow::BannerFiller => None,
                    DisplayRow::Item(idx) => Some(*idx),
                })
                .collect();

            let mut state = ListState::default();
            state.select(Some(display_cursor.saturating_sub(offset)));
            layout.cursor_screen_y =
                Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            if banner_rows > 0 {
                let banner_start = display_cursor + 1;
                if banner_start >= offset && banner_start < offset + visible {
                    let banner_y = content_area.y + (banner_start - offset) as u16;
                    let bottom = content_area.y + content_area.height;
                    let banner_h =
                        (COMPACT_BANNER_CONTENT_ROWS as u16).min(bottom.saturating_sub(banner_y));
                    if banner_h > 0 {
                        let banner_rect = Rect {
                            x: content_area.x,
                            y: banner_y,
                            width: content_area.width,
                            height: banner_h,
                        };
                        // render_power_compact_detail overwrites layout.cursor_screen_y with
                        // the banner's own top row; restore the selected list row's y after,
                        // since that row (not the banner) is what should host the blinking
                        // cursor / mouse hit target.
                        let want_cursor_y = layout.cursor_screen_y;
                        self.render_power_compact_detail(
                            f,
                            banner_rect,
                            self.power_left_tab - 1,
                            focused,
                            layout,
                        );
                        layout.cursor_screen_y = want_cursor_y;
                    }
                }
            }

            if focused && total_display > visible {
                let max_off = total_display.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None)
                        .end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area,
                    &mut sb,
                );
            }
        }
```

- [ ] **Step 4: Add test helpers used in Step 1**

Add to the `tests` module in `src/app/render/power/list.rs` (match this file's existing movie-fixture/render-to-string helper signatures — check `letter_group_keeps_top_bucket_header_after_scrolling_back_to_top` in this same file for the exact conventions before finalizing field names):

```rust
fn make_power_movie_list_app(titles: Vec<&str>) -> App {
    let mut app = make_app_stub();
    app.power_left_tab = 1;

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let items: Vec<_> = titles
        .into_iter()
        .enumerate()
        .map(|(i, title)| {
            let mut m = make_item(title, "Movie");
            m.id = format!("movie-{i}");
            if title.contains("Selected") {
                m.overview = "This is the compact movie banner overview text.".into();
            }
            m
        })
        .collect();
    let total = items.len();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items,
            total_count: total,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
        }],
        search: None,
        feed_home_video: None,
        power_detail_item: None,
        power_detail_scroll: 0,
    });

    app
}

fn render_power_list_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPower::default();
    term.draw(|f| {
        app.render_power_list(f, Rect::new(0, 0, width, height), true, &mut layout);
    })
    .unwrap();
    let buffer = term.backend().buffer().clone();
    buffer_to_string(&buffer) // reuse this file's existing buffer-to-string helper if present under a different name
}
```

If this file already has a `render_power_list`-to-string or buffer-to-string helper under a different name (check the existing letter-group/artist-group tests before adding a duplicate), reuse it instead of adding a new one.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib compact_banner_appears_under_selected_row_not_pinned_to_top`
Expected: PASS

- [ ] **Step 6: Run full list.rs test module to check no regressions**

Run: `cargo test --lib render::power::list::tests`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/app/render/power/list.rs
git commit -m "Render the compact movie banner inline under the selected row in the plain list branch"
```

---

## Task 4: Interleave banner filler rows in the letter-grouped list branch

**Files:**
- Modify: `src/app/render/power/list.rs`
- Test: `src/app/render/power/list.rs` (`tests` module)

**Security flag:** `none`

**Does NOT cover:** The artist-grouped (`show_grouped`, music-album-folder) branch — movies never reach that branch (it requires `is_viewing_album_folders`, which only applies to music libraries), so it's intentionally left untouched.

**Does NOT cover:** Non-movie libraries with 50+ items — `banner_rows` is 0 for every library type except "leaf movie selected, detail not pinned" (Task 2's `compact_banner_rows` guard), so this change is inert for TV/music/home-video letter-grouped lists.

- [ ] **Step 1: Write failing test — banner follows cursor in a 50+-item letter-grouped movie library**

Add to `src/app/render/power/list.rs`'s `tests` module:

```rust
#[test]
fn compact_banner_appears_inline_in_letter_grouped_movie_list() {
    // 60 items forces use_letter_groups (>= 50, non-music collection_type).
    let titles: Vec<String> = (0..60).map(|i| format!("Movie {i:02}")).collect();
    let mut app = make_power_movie_list_app(titles.iter().map(String::as_str).collect());
    // Select an item partway through so the banner must appear mid-list, not at row 0.
    app.libs[0].nav_stack.last_mut().unwrap().cursor = 10;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .get_mut(10)
        .unwrap()
        .overview = "This is the compact movie banner overview text.".into();

    let out = render_power_list_to_string(&mut app, 40, 30);
    assert!(
        out.contains("compact movie banner"),
        "expected banner content to render somewhere in the letter-grouped list:\n{out}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib compact_banner_appears_inline_in_letter_grouped_movie_list`
Expected: FAIL — letter-grouped branch doesn't insert or draw banner filler rows yet, so `out` never contains the banner overview text (scroll window shows only plain item rows).

- [ ] **Step 3: Implement the letter-grouped branch change**

In the `use_letter_groups` branch of `render_power_list`, make four changes:

**3a.** Add a `BannerFiller` variant to the branch's local `DisplayRow` enum. Replace:

```rust
            enum DisplayRow {
                Spacer,
                LetterHeader(String),
                Item(usize),
            }
```

with:

```rust
            enum DisplayRow {
                Spacer,
                LetterHeader(String),
                Item(usize),
                BannerFiller,
            }
```

**3b.** Insert filler rows right after the selected item's row while building `display_rows`. Replace:

```rust
            let mut display_rows: Vec<DisplayRow> = Vec::new();
            let mut last_bucket = String::new();
            for &idx in &sorted_indices {
                let item = &items[idx];
                let bucket = letter_bucket(item, total_count);
                if bucket != last_bucket {
                    if !last_bucket.is_empty() {
                        display_rows.push(DisplayRow::Spacer);
                    }
                    display_rows.push(DisplayRow::LetterHeader(bucket.clone()));
                    last_bucket = bucket;
                }
                display_rows.push(DisplayRow::Item(idx));
            }
```

with:

```rust
            let mut display_rows: Vec<DisplayRow> = Vec::new();
            let mut last_bucket = String::new();
            for &idx in &sorted_indices {
                let item = &items[idx];
                let bucket = letter_bucket(item, total_count);
                if bucket != last_bucket {
                    if !last_bucket.is_empty() {
                        display_rows.push(DisplayRow::Spacer);
                    }
                    display_rows.push(DisplayRow::LetterHeader(bucket.clone()));
                    last_bucket = bucket;
                }
                display_rows.push(DisplayRow::Item(idx));
                if banner_rows > 0 && idx == cursor {
                    for _ in 0..banner_rows {
                        display_rows.push(DisplayRow::BannerFiller);
                    }
                }
            }
```

**3c.** Extend the offset's lower bound the same way as the plain branch, keeping the existing header-backup nudge. Replace:

```rust
            let mut offset = stored_scroll.clamp(
                display_cursor.saturating_sub(visible.saturating_sub(1)),
                display_cursor,
            );
```

with:

```rust
            let lower_bound = (display_cursor + banner_rows)
                .saturating_sub(visible.saturating_sub(1))
                .min(display_cursor);
            let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
```

**3d.** Extend the `left_row_map` match and the `list_items` match to handle `BannerFiller`, and draw the banner after the list renders. Replace:

```rust
            for row in display_rows.iter().skip(offset).take(visible) {
                layout.left_row_map.push(match row {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) => None,
                    DisplayRow::Item(idx) => Some(*idx),
                });
            }
```

with:

```rust
            for row in display_rows.iter().skip(offset).take(visible) {
                layout.left_row_map.push(match row {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) | DisplayRow::BannerFiller => {
                        None
                    }
                    DisplayRow::Item(idx) => Some(*idx),
                });
            }
```

Replace:

```rust
                .map(|row| match row {
                    DisplayRow::Spacer => ListItem::new(Line::default()),
                    DisplayRow::LetterHeader(label) => ListItem::new(Line::from(vec![
```

with:

```rust
                .map(|row| match row {
                    DisplayRow::Spacer | DisplayRow::BannerFiller => ListItem::new(Line::default()),
                    DisplayRow::LetterHeader(label) => ListItem::new(Line::from(vec![
```

Then, immediately after this branch's `f.render_stateful_widget(List::new(list_items)..., content_area, &mut state);` call and before its `if focused && total_display > visible { ... }` scrollbar block, insert:

```rust
            if banner_rows > 0 {
                let banner_start = display_cursor + 1;
                if banner_start >= offset && banner_start < offset + visible {
                    let banner_y = content_area.y + (banner_start - offset) as u16;
                    let bottom = content_area.y + content_area.height;
                    let banner_h =
                        (COMPACT_BANNER_CONTENT_ROWS as u16).min(bottom.saturating_sub(banner_y));
                    if banner_h > 0 {
                        let banner_rect = Rect {
                            x: content_area.x,
                            y: banner_y,
                            width: content_area.width,
                            height: banner_h,
                        };
                        let want_cursor_y = layout.cursor_screen_y;
                        self.render_power_compact_detail(
                            f,
                            banner_rect,
                            self.power_left_tab - 1,
                            focused,
                            layout,
                        );
                        layout.cursor_screen_y = want_cursor_y;
                    }
                }
            }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib compact_banner_appears_inline_in_letter_grouped_movie_list`
Expected: PASS

- [ ] **Step 5: Run full list.rs test module to check no regressions**

Run: `cargo test --lib render::power::list::tests`
Expected: PASS — including pre-existing letter-group tests like `letter_group_keeps_top_bucket_header_after_scrolling_back_to_top` (must still pass since `banner_rows == 0` for non-movie / no-selection cases, which is what those tests exercise).

- [ ] **Step 6: Commit**

```bash
git add src/app/render/power/list.rs
git commit -m "Render the compact movie banner inline in the letter-grouped list branch"
```

---

## Task 5: Delete render_power_library's now-redundant top-pinned banner special-case

**Files:**
- Modify: `src/app/render/power/mod.rs`
- Test: `src/app/render/power/mod.rs` (`tests` module)

**Security flag:** `none`

**Does NOT cover:** `has_detail` (expanded detail) handling, or the `is_feed_group`/`is_music_group`/`is_album`/`is_series`/`is_home_video` branches — all unchanged, still evaluated before the catch-all `else`.

- [ ] **Step 1: Confirm the existing regression test still describes correct behavior**

`movie_library_renders_compact_banner_without_opening_expanded_detail` (already in `src/app/render/power/mod.rs`, lines 617-651) exercises a movie library with the cursor on item 0 of 2 items — no scrolling needed. Because the selected item's row now sits at the same screen position the old `top_area` paragraph used to occupy (both are row 0 of the panel), and Tasks 3-4 reserve the identical `COMPACT_BANNER_TOTAL_ROWS` (14 = 13 content + 1 gap) immediately after it, this test's line-index assertions (`lines[0]` = title, `lines[14]` = blank gap, `lines[15]` = "Second Movie") are expected to keep passing unchanged after this task's deletion. Do not edit this test yet — run it after Step 2 and only touch it if it fails.

- [ ] **Step 2: Delete the show_compact_movie_detail branch and its local consts**

In `render_power_library` (`src/app/render/power/mod.rs`), delete the `show_compact_movie_detail` computation. Replace:

```rust
        let show_compact_movie_detail =
            !has_detail && self.power_selected_movie_item(lib_idx).is_some();
        if has_detail {
```

with:

```rust
        if has_detail {
```

Then delete the entire `else if show_compact_movie_detail { ... }` block that follows the `has_detail` branch (from `} else if show_compact_movie_detail {` through its matching closing `}` — the block containing `TOP_ITEM_ROW_H`, `COMPACT_DETAIL_H`, `COMPACT_DETAIL_GAP`, the `top_area`/`banner_area`/`list_area` split, the manual title-line rendering, the `restore_search`/`restore_nav` cursor-nudge hack, and the nested `render_power_list` call), so the branch chain becomes:

```rust
        if has_detail {
            self.render_power_detail(f, area, lib_idx, focused, layout);
        } else if is_feed_group {
            self.render_power_feed_home_video_group_view(f, area, lib_idx, focused, layout);
        } else if is_music_group {
            self.render_power_music_group_view(f, area, lib_idx, focused, layout);
        } else if is_album {
            self.render_power_album_detail(f, area, lib_idx, focused, layout);
        } else if is_series {
            self.render_power_episode_detail(f, area, lib_idx, focused, layout);
        } else if is_home_video {
            self.render_power_home_video_list(f, area, lib_idx, focused, layout);
        } else {
            self.render_power_list(f, area, focused, layout);
        }
```

- [ ] **Step 3: Run the pre-existing regression test**

Run: `cargo test --lib movie_library_renders_compact_banner_without_opening_expanded_detail`
Expected: PASS (per Step 1's reasoning). If it fails, inspect the actual output string in the assertion failure message and adjust only the specific line-index assertions that differ — do not change the test's intent (verifying the banner shows inline with the list still visible below it).

- [ ] **Step 4: Run the full mod.rs test module**

Run: `cargo test --lib render::power::mod::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app/render/power/mod.rs
git commit -m "Remove the top-pinned compact-banner special case now that render_power_list handles it inline"
```

---

## Task 6: Fix left-panel search-result click handling to respect left_row_map

**Files:**
- Modify: `src/app/input.rs`

**Security flag:** `none`

**Does NOT cover:** The feed-group click branch (already correctly checks `use_row_map`) or the nav_stack click branch (already correctly checks `use_row_map`) — only the search-result branch, which currently ignores `left_row_map` unconditionally.

- [ ] **Step 1: Write failing test — clicking below the banner in movie search results selects the right result, not an off-by-banner-rows result**

Add to the `tests` module in `src/app/input.rs` (match this file's existing power-view mouse-click test conventions):

```rust
#[test]
fn click_in_movie_search_results_below_banner_selects_correct_result() {
    let mut app = make_power_movie_search_app(); // movies library with `search` set, 3 results,
                                                   // cursor on result 0 so the banner reserves
                                                   // COMPACT_BANNER_TOTAL_ROWS rows after row 0
    app.power_focus = PowerFocus::Left;
    render_power_view_for_test(&mut app, 60, 30); // populates layout.power.left_row_map via a real render pass

    // Row 15 (0 of 3, offset14 gap+banner, per Task 3/5's fixed geometry) should
    // map to search result index 1, not index 1 shifted by the banner's rows.
    let la = app.layout.power.left_area;
    let click_row = la.y + 15;
    app.handle_mouse_click(la.x + 2, click_row); // use this file's actual mouse-click entry point name

    let search = app.libs[0].search.as_ref().unwrap();
    assert_eq!(search.cursor, 1, "click should select the second search result");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib click_in_movie_search_results_below_banner_selects_correct_result`
Expected: FAIL — the search click branch recomputes its own offset assuming 1:1 row-to-result mapping, so it selects the wrong result once banner filler rows shift the mapping.

- [ ] **Step 3: Implement the fix**

In `src/app/input.rs`, replace the search-result click branch:

```rust
                    if let Some(s) = &mut lib.search {
                        let visible = la.height as usize;
                        let offset = if s.cursor >= visible {
                            s.cursor - visible + 1
                        } else {
                            0
                        };
                        let clicked = offset + click_y;
                        if clicked < s.results.len() {
                            s.cursor = clicked;
                        }
                    } else if is_feed_group {
```

with:

```rust
                    if let Some(s) = &mut lib.search {
                        if use_row_map {
                            // Letter-grouped or banner-adjacent mode: row map gives the
                            // result index directly (None = header/banner-filler row).
                            if let Some(Some(item_idx)) = row_map_item {
                                if item_idx < s.results.len() {
                                    s.cursor = item_idx;
                                }
                            }
                        } else {
                            let visible = la.height as usize;
                            let offset = if s.cursor >= visible {
                                s.cursor - visible + 1
                            } else {
                                0
                            };
                            let clicked = offset + click_y;
                            if clicked < s.results.len() {
                                s.cursor = clicked;
                            }
                        }
                    } else if is_feed_group {
```

- [ ] **Step 4: Add test helpers used in Step 1**

Add `make_power_movie_search_app` and confirm the actual mouse-click handler name (check existing power-view click tests in `src/app/input.rs`'s `tests` module for the real entry point — likely a method on `App` taking column/row or a `MouseEvent`; use whatever this file's existing click tests already call rather than inventing `handle_mouse_click`).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib click_in_movie_search_results_below_banner_selects_correct_result`
Expected: PASS

- [ ] **Step 6: Run full input.rs test module to check no regressions**

Run: `cargo test --lib input::tests`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/app/input.rs
git commit -m "Fix left-panel search-result clicks to use left_row_map so banner filler rows don't shift click targets"
```

---

## Task 7: Full verification pass

**Files:** none (verification only)

**Security flag:** `none`

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: PASS, zero failures.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings/errors.

- [ ] **Step 3: Run GitNexus impact + detect_changes per repo convention**

Since this touches `render_power_card`, `render_power_list`, `render_power_library`, and the left-panel input handler — all previously flagged HIGH-traffic symbols in AGENTS.md — run:

```
detect_changes({scope: "compare", base_ref: "main"})
```

Expected: the affected-symbol list matches exactly the functions touched in Tasks 1-6 (`render_power_card`, `render_power_list`, `render_power_library`, the left-panel mouse click handler) plus their new test functions — no unexpected symbols flagged.

- [ ] **Step 4: Manual visual check (per project convention — Power View changes are high-risk, verify visually not just via tests)**

Launch the app against a movies library with 50+ items (to exercise the letter-grouped branch) and confirm:
- Selecting a leaf movie shows the banner directly under its row, list scrolling naturally as the cursor moves.
- Scrolling near the bottom of the list keeps the full banner visible until the viewport is truly too short.
- The left card panel shows the movie's backdrop/poster while a movie is selected (library focused), reverts to queue art when focus moves to the queue, and reverts to queue art (not blank) for a movie with no images.
- `Alt+M` still opens/closes expanded detail correctly; expanded detail still shows the larger poster, director, and scrollable overview unchanged.
