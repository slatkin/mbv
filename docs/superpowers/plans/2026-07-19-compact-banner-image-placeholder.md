# Movie Compact Banner Image Placeholder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reserve a fixed-size dim placeholder box for the movie compact banner's poster image while it's loading, instead of collapsing to zero width and reflowing the overview/director text once the image arrives.

**Architecture:** Mirror the pattern the episode banner (`src/app/render/power/episode.rs`) already uses for its series-image placeholder: track whether the poster fetch is genuinely absent (no space reserved) vs. in flight (reserve the same `IMG_COLS x IMG_ROWS` box the loaded image would use, and paint it with a dim `palette::OVERLAY` block instead of the real `StatefulImage` widget). All changes are confined to `src/app/render/power/detail.rs`.

**Tech Stack:** Rust, ratatui, ratatui-image.

## Global Constraints

- No new constants — reuse the existing `IMG_COLS: u16 = 18` and `IMG_ROWS: u16 = 12` already defined at the top of `src/app/render/power/detail.rs`.
- Surgical change: touch only `src/app/render/power/detail.rs`. Do not modify `episode.rs`, `card.rs`, or `list.rs` — `episode.rs` is reference precedent only, not a dependency.
- Follow this repo's pre-commit checklist (`docs/CHECKIN.md`): `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` must all be clean before committing.
- Never add `Co-Authored-By:` trailers to commit messages.

---

### Task 1: Reserve and render a placeholder while the movie poster loads

**Files:**
- Modify: `src/app/render/power/detail.rs`

**Interfaces:**
- Consumes: existing `App` fields `card_image_loading: HashSet<String>`, `card_image_states: HashMap<String, Option<ThreadProtocol>>`, methods `images_enabled(&self) -> bool`, `list_image_renders_allowed(&self) -> bool`, `fetch_card_image(&mut self, cache_key: String, item_id: String, series_id: String, types: &[&str])`, function `compact_banner_image_cache_key(item_id: &str) -> String` (all already defined in this file / `images.rs`).
- Produces: new field `CompactBannerLayout.img_is_placeholder: bool` — internal to this file, not consumed elsewhere.

- [x] **Step 1: Write the failing test**

Add this test to the `#[cfg(test)] mod tests` block at the bottom of `src/app/render/power/detail.rs`, right after `compact_movie_detail_shows_full_short_overview_with_no_scrollbar` (before `compact_movie_detail_shows_full_long_overview_with_no_scrollbar`):

```rust
    // The poster fetch is triggered synchronously inside `compact_banner_layout`
    // but resolves asynchronously on a background thread; nothing drains that
    // result in this test, so right after the render the fetch is still "in
    // flight" (`card_image_loading` contains the key, `card_image_states`
    // does not yet). The banner must reserve the same IMG_COLS x IMG_ROWS box
    // the loaded image would use, not collapse to zero width.
    #[test]
    fn compact_movie_detail_reserves_placeholder_space_while_image_loads() {
        let mut app = make_app_stub();
        app.image_protocol_enabled = true;

        let mut movie = make_item("Focused Movie", "Movie");
        movie.id = "movie-1".into();
        movie.overview = "A short overview.".into();
        push_movie_lib(&mut app, movie);

        let mut layout = LayoutPower::default();
        let out = render_power_compact_detail_to_string(&mut app, &mut layout);

        assert!(
            app.card_image_loading.contains("movie-1:cmp_primary"),
            "expected the poster fetch to have been triggered and still be in flight"
        );
        assert!(
            !app.card_image_states.contains_key("movie-1:cmp_primary"),
            "fetch must not have resolved yet for this assertion to be meaningful"
        );
        assert_eq!(
            layout.inline_image_rect.map(|r| (r.width, r.height)),
            Some((18, 12)),
            "expected the placeholder to reserve the banner's IMG_COLS x IMG_ROWS box:\n{out}"
        );
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --workspace compact_movie_detail_reserves_placeholder_space_while_image_loads`
Expected: FAIL — the assertion on `layout.inline_image_rect` fails because the current code reserves `(0, 0)` (i.e. `inline_image_rect` is `None`) while the fetch is in flight, instead of `(18, 12)`.

- [x] **Step 3: Add the `img_is_placeholder` field to `CompactBannerLayout`**

In `src/app/render/power/detail.rs`, find this struct definition:

```rust
pub(super) struct CompactBannerLayout {
    meta_line: Option<String>,
    show_playing: bool,
    /// Wrapped overview lines, plus (if there's a director) a blank
    /// separator line and a placeholder line at `director_line_idx` that
    /// renders as "Director: <name>" instead of plain text.
    lines: Vec<String>,
    director_line_idx: Option<usize>,
    img_actual_w: u16,
    img_height: u16,
}
```

Replace it with:

```rust
pub(super) struct CompactBannerLayout {
    meta_line: Option<String>,
    show_playing: bool,
    /// Wrapped overview lines, plus (if there's a director) a blank
    /// separator line and a placeholder line at `director_line_idx` that
    /// renders as "Director: <name>" instead of plain text.
    lines: Vec<String>,
    director_line_idx: Option<usize>,
    img_actual_w: u16,
    img_height: u16,
    /// True when `img_actual_w`/`img_height` describe a reserved-but-not-yet-
    /// loaded box (fetch in flight, or resize+encode still running on the
    /// worker thread) rather than a real decoded image. The render pass uses
    /// this to draw a dim placeholder block instead of `StatefulImage`.
    img_is_placeholder: bool,
}
```

- [x] **Step 4: Update `compact_banner_layout` to compute and return `img_is_placeholder`**

Find this block (still in `src/app/render/power/detail.rs`, inside `pub(super) fn compact_banner_layout`):

```rust
        let primary_cache_key = compact_banner_image_cache_key(&item.id);
        if self.images_enabled() {
            self.fetch_card_image(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        let (img_actual_w, img_height): (u16, u16) = {
            if self.list_image_renders_allowed() {
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    // `size_for` is `None` while resize+encode is in-flight on
                    // the worker thread; treat that the same as no image yet.
                    let actual = state
                        .size_for(
                            ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                            ratatui::layout::Size {
                                width: IMG_COLS,
                                height: IMG_ROWS,
                            },
                        )
                        .unwrap_or_default();
                    (actual.width, actual.height)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            }
        };
```

Replace it with:

```rust
        let primary_cache_key = compact_banner_image_cache_key(&item.id);
        if self.images_enabled() {
            self.fetch_card_image(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        // True while the raw image fetch itself is in flight (before it
        // lands in `card_image_states`, success or failure). Used below to
        // reserve a placeholder box the same size as the eventual image
        // instead of collapsing text to full width and then narrowing it
        // once the image arrives -- mirrors the pattern already used by the
        // episode banner's series-image placeholder (episode.rs).
        let img_loading =
            self.images_enabled() && self.card_image_loading.contains(&primary_cache_key);

        let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) = {
            if self.list_image_renders_allowed() {
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    // `size_for` is `None` while resize+encode is in-flight on
                    // the worker thread; treat that the same as still loading.
                    match state.size_for(
                        ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                        ratatui::layout::Size {
                            width: IMG_COLS,
                            height: IMG_ROWS,
                        },
                    ) {
                        Some(actual) => (actual.width, actual.height, false),
                        None => (IMG_COLS, IMG_ROWS, true),
                    }
                } else if img_loading {
                    (IMG_COLS, IMG_ROWS, true)
                } else {
                    (0, 0, false)
                }
            } else {
                (0, 0, false)
            }
        };
```

Then find the struct literal at the end of the same function:

```rust
        CompactBannerLayout {
            meta_line,
            show_playing,
            lines,
            director_line_idx,
            img_actual_w,
            img_height,
        }
    }
```

Replace it with:

```rust
        CompactBannerLayout {
            meta_line,
            show_playing,
            lines,
            director_line_idx,
            img_actual_w,
            img_height,
            img_is_placeholder,
        }
    }
```

- [x] **Step 5: Render the placeholder block in `render_power_compact_detail`**

Find this line pair near the top of `render_power_compact_detail`:

```rust
        let img_actual_w = content.img_actual_w;
        let img_height = content.img_height;
```

Replace it with:

```rust
        let img_actual_w = content.img_actual_w;
        let img_height = content.img_height;
        let img_is_placeholder = content.img_is_placeholder;
```

Then find the final image-render block near the end of the same function:

```rust
        if img_height > 0 {
            let primary_cache_key = compact_banner_image_cache_key(&item.id);
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
                    Rect {
                        x: img_x,
                        y: img_y,
                        width: img_actual_w,
                        height: img_height,
                    },
                    state,
                );
            }
        }
    }
}
```

Replace it with:

```rust
        if img_height > 0 {
            let img_rect = Rect {
                x: img_x,
                y: img_y,
                width: img_actual_w,
                height: img_height,
            };
            if img_is_placeholder {
                // Image still loading -- draw a dim placeholder block to
                // hold the space (mirrors episode.rs's series-image
                // placeholder).
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::OVERLAY)),
                    img_rect,
                );
            } else {
                let primary_cache_key = compact_banner_image_cache_key(&item.id);
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                    f.render_stateful_widget(
                        SImg::default()
                            .resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
                        img_rect,
                        state,
                    );
                }
            }
        }
    }
}
```

- [x] **Step 6: Fix the pre-existing `CompactBannerLayout` struct literals in the `content_rows` test**

The new `img_is_placeholder` field makes every existing `CompactBannerLayout { ... }` literal in the test module fail to compile. In `src/app/render/power/detail.rs`, inside `fn content_rows_is_never_shorter_than_the_rendered_image_height`, update all three literals:

```rust
        let short_text_layout = CompactBannerLayout {
            meta_line: None,
            show_playing: false,
            lines: vec!["A short overview.".to_string()],
            director_line_idx: None,
            img_actual_w: 18,
            img_height: 12,
        };
```
becomes
```rust
        let short_text_layout = CompactBannerLayout {
            meta_line: None,
            show_playing: false,
            lines: vec!["A short overview.".to_string()],
            director_line_idx: None,
            img_actual_w: 18,
            img_height: 12,
            img_is_placeholder: false,
        };
```

```rust
        let tall_text_layout = CompactBannerLayout {
            meta_line: Some("Crime  1974  1h33m".to_string()),
            show_playing: false,
            lines: vec!["line".to_string(); 20],
            director_line_idx: None,
            img_actual_w: 18,
            img_height: 12,
        };
```
becomes
```rust
        let tall_text_layout = CompactBannerLayout {
            meta_line: Some("Crime  1974  1h33m".to_string()),
            show_playing: false,
            lines: vec!["line".to_string(); 20],
            director_line_idx: None,
            img_actual_w: 18,
            img_height: 12,
            img_is_placeholder: false,
        };
```

```rust
        let no_image_layout = CompactBannerLayout {
            meta_line: None,
            show_playing: false,
            lines: vec!["A short overview.".to_string()],
            director_line_idx: None,
            img_actual_w: 0,
            img_height: 0,
        };
```
becomes
```rust
        let no_image_layout = CompactBannerLayout {
            meta_line: None,
            show_playing: false,
            lines: vec!["A short overview.".to_string()],
            director_line_idx: None,
            img_actual_w: 0,
            img_height: 0,
            img_is_placeholder: false,
        };
```

- [x] **Step 7: Run the new test to verify it passes**

Run: `cargo test --workspace compact_movie_detail_reserves_placeholder_space_while_image_loads`
Expected: PASS

- [x] **Step 8: Run the full pre-existing test suite for this file to check for regressions**

Run: `cargo test --workspace compact_movie_detail`
Expected: all matching tests in `src/app/render/power/detail.rs` PASS, including `compact_movie_detail_shows_director_without_enter_prompt`, `compact_movie_detail_shows_full_short_overview_with_no_scrollbar`, `compact_movie_detail_shows_full_long_overview_with_no_scrollbar`, and the new `compact_movie_detail_reserves_placeholder_space_while_image_loads`. Then also run `cargo test --workspace content_rows_is_never_shorter_than_the_rendered_image_height` to confirm that struct-literal fix compiles and passes.

- [x] **Step 9: Run the full pre-commit checklist**

Run, in order:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```
Expected: all three commands exit clean (no diffs, no warnings, all tests pass).

- [x] **Step 10: Commit**

```bash
git add src/app/render/power/detail.rs
git commit -m "$(cat <<'EOF'
feat: reserve placeholder space for loading movie banner poster

The compact movie banner collapsed its poster image's reserved space to
zero width/height while the fetch was in flight, so the overview/director
text laid out at full banner width and then reflowed narrower the instant
the image arrived. Now reserves the same IMG_COLS x IMG_ROWS box the
loaded image would use and fills it with a dim placeholder block,
mirroring the pattern episode.rs already uses for its series-image
placeholder.
EOF
)"
```
