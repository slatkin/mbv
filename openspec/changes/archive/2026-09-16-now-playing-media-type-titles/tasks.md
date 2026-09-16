## 1. Shared title-part mapping (`crates/mbv-core`)

- [x] 1.1 Add a typed title-parts value (a title part, an optional context part, and a closed
  part role) plus the media-type mapping to `crates/mbv-core/src/playback/queue_items.rs`,
  covering Emby movie / episode / audio track / home video, Audiobookshelf podcast episode and
  book, and feed entry. The mapping takes the caller-resolved feed subscription name as an
  explicit optional input and reads it only for feed items. Verify with a named `#[case]` table
  in the existing core test module (`crates/mbv-core/src/playback/tests/`) named per media type
  and run `cargo nextest run -p mbv-core`.

- [x] 1.2 In the same table, cover the degradation cases that must produce a single part: an
  Audiobookshelf podcast episode with no show title, a feed entry with no matching subscription,
  a feed entry with no `feed_id`, an Audiobookshelf book, and an Emby movie / home video. Verify
  each case asserts exactly one part and no context part.

- [x] 1.3 Confirm the adjacent presentation methods are untouched and still pass:
  `QueueItem::display_name()`, `QueueItem::display_name_parts()`, `EmbyItem::playback_label()`
  and `EmbyItem::display_name()` carry no changes. Verify with
  `cargo nextest run -p mbv-core` including `api_tests_parsing.rs`.

## 2. Theme roles

- [x] 2.1 Add two role-specific primitives in `src/app/render/theme/primitives.rs` (the aqua
  title value and the yellow context value), each with a doc comment recording that it shares a
  value with the brand aqua / focused-row accent but is deliberately its own primitive. Verify by
  inspecting that no existing role or primitive references the new names.

- [x] 2.2 Add the two roles in `src/app/render/theme/mod.rs` and re-export them through
  `src/app/palette.rs`. Verify `cargo check -p mbv` and that the role comments state what each
  means (now-playing title / now-playing context) rather than naming a hue.

- [x] 2.3 Add the single role-to-colour resolution point in the playback painter
  (`src/app/render/components/chrome_player.rs`) so no other site maps a part role to a palette
  role. Verify with a painter buffer test asserting both role cells' `fg`.

## 3. Projection and wiring

- [x] 3.1 Change the shell→component title payload from `Vec<(String, Color)>` to the typed parts
  carrying a closed role, in `PlaybackProjection`
  (`src/app/components/library_playback_panel.rs`) and `PlaybackRenderContext`
  (`src/app/render/components/chrome_player.rs`). Verify `cargo check -p mbv` and that the two
  panels' test fixtures still construct a projection.

- [x] 3.2 Resolve the feed subscription display name in the App layer by matching
  `FeedEntry.feed_id` against the configured feed subscriptions' urls, and pass it into the
  mapping. Verify with a unit test covering a matching subscription, a non-matching entry and a
  `None` `feed_id`, asserting the resolved name is `None` in the last two cases.

- [x] 3.3 Rebuild `Model::transport_projection` (`src/app/shell_playback.rs`) from the queue item
  rather than a title string, and remove the `display_name() == title` re-match plus the `&mut`
  receiver and the by-value clones it forced in
  `App::playback_title_parts` (`src/app/render/components/chrome_player_context.rs`). Verify
  `cargo nextest run -p mbv` and that no remaining call site clones a title to satisfy the
  borrow checker.

## 4. Panel behaviour

- [x] 4.1 Add buffer-level tests in `src/app/components/library_playback_panel.rs` and
  `src/app/components/queue_playback_panel.rs` covering each media type in the requirements
  table: two-part rows paint the title part in the aqua title role and the context part in the
  yellow context role; single-part rows paint wholly in the title role. Verify the assertions are
  on painted cell foregrounds, not on the projected values.

- [x] 4.2 Assert the delineation is a single space and nothing else: the painted row text has
  exactly one space between the parts and contains no `-`, `–`, `—`, `|` or `•` between them.
  Verify by asserting on the buffer's row text for every two-part media type.

- [x] 4.3 Assert the roles survive the overflow marquee: paint a two-part title wider than its
  slot and verify the marqueed window keeps both role colours. Verify in the painter's own tests
  in `src/app/render/components/chrome_player.rs`.

- [x] 4.4 Assert the audiobook row paints the book title alone, with no context part, matching
  design D5. Verify with a buffer-level case in the panel tests.

## 5. Regression and integration

- [x] 5.1 Confirm media lists did not move: the existing two-tone list-row tests in
  `src/app/render/components/media_list/` and
  `src/app/render/components/media_list.rs` pass unmodified. Verify with
  `cargo nextest run -p mbv` and no edits to those test files.

- [x] 5.2 Add one real `Application::tick()` integration test through the shell sync pass
  (`src/app/tests_tick_integration*.rs`) asserting the projected parts for a two-part item and a
  single-part item, proving both panels receive them from the one projection. Verify with
  `cargo nextest run -p mbv`.

- [x] 5.3 Run the gates: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run -p mbv` and `cargo nextest run -p mbv-core`. Verify all clean.

- [x] 5.4 Manual check, not a test: on a real terminal, confirm the aqua title reads clearly on
  the focused Library strip (the measured ~3.17:1 case) in the narrow and wide layouts, and that
  the one-space colour delineation reads as two parts rather than one run-together title. Record
  the outcome; if it is unreadable, the follow-up is a lighter aqua primitive, not a different
  role.
