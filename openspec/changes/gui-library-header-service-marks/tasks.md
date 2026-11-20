# Tasks

## 1. Feed platform in `mbv-feed`

- [x] 1.1 Add `crates/mbv-feed/src/platform.rs` with `FeedPlatform` and `FeedPlatform::of_url` as design D1 states: the domain-suffix host rule for the nine host platforms, then the PeerTube path-and-parameter rule. In `lib.rs`, add `mod platform;`, add `pub use self::platform::FeedPlatform;`, and make `url_authority_and_path` `pub(crate)`. Do not change `normalize_feed_url`. Verify: `cargo check -p mbv-feed`.
- [x] 1.2 Add `rstest.workspace = true` under `[dev-dependencies]` in `crates/mbv-feed/Cargo.toml` (the workspace pin, `rstest` 0.28). Add `crates/mbv-feed/tests/feed_platform.rs` with one named `#[case]` table that owns the contract "a URL shows its Feed platform": one case for each platform's feed form in the D1 table, a PeerTube channel feed on a self-hosted instance, a PeerTube `playlistId` podcast feed, a PeerTube instance-wide feed with no filter (`None`), `https://notyoutube.com/feed` (`None`), and a generic podcast host (`None`). Verify: `cargo nextest run -p mbv-feed`.

## 2. Checked-platform rule in the core

- [x] 2.1 Add `pub fn gui_feeds_platform(collections: &GuiCollections) -> Option<FeedPlatform>` to `src/app/gui_library/feeds_projection.rs` as design D2 states. Re-export it beside `GuiFeedsProjection` in `src/app/gui_library.rs`, `src/app.rs`, and `src/lib.rs`. Also re-export `mbv_feed::FeedPlatform` from `src/lib.rs`. Verify: `cargo check -p mbv`.
- [x] 2.2 In `src/app/tests/gui_library_feeds.rs`, add the D6 core tests through the existing App fixture and `toggle_subscription`. Seed one YouTube URL and one generic URL with `seed_gui_feeds_for_test` in a new helper. Add a two-case named table (only YouTube checked → `Some(FeedPlatform::YouTube)`; YouTube plus generic checked → `None`) and one test (All over YouTube-only subscriptions → `None`). Keep the file under 800 lines. Verify: `cargo nextest run -p mbv gui_library_feeds`.

## 3. Feeds frame carries the platform

- [x] 3.1 Add `pub platform: Option<FeedPlatform>` to `GuiFeedsList` in `crates/mbv-gui/src/bridge/frame/library/feeds.rs`. Set it in `from_projection` from `mbv::gui_feeds_platform(&projection.collections)`, as design D3 states. Add `platform: None` to the six test struct literals: `window/library/status/tests.rs`, `window/library/feeds_keys/tests.rs`, `window/components/library_status_bar/tests.rs`, `window/components/library_feeds_list/tests.rs`, `window/components/library_feeds_hero/tests.rs`, and `window/components/library_panel/tests/fixtures.rs`. Verify: `cargo check -p mbv-gui --all-targets`.

## 4. Service mark painter

- [ ] 4.1 Add `crates/mbv-gui/src/window/service_mark.rs` (registered in `window.rs`) with `ServiceMark`, `ServiceMark::feeds`, `pub(crate) const EMBY_PATH`, and the `ServiceMarkIcon` component, as design D4 states. Copy each path from `https://cdn.jsdelivr.net/npm/simple-icons@16.38.0/icons/<slug>.svg` for the slugs and colours in the D4 table. Paint RSS through the Lucide `Rss` component. Paint Patreon and RSS in `var(--canvas-heading)`. Use only exhaustive matches. Make `TargetBrand::Emby` in `brand_icons.rs` read `EMBY_PATH`, and name the new module in the `brand_icons.rs` module doc. Verify: `cargo check -p mbv-gui`, and the existing `brand_icons` and queue chooser tests pass with `cargo nextest run -p mbv-gui queue_chooser queue_footer`.

## 5. Header takes a heading

- [ ] 5.1 In `components/library_list_header.rs`, replace `title` and `title_attr` with `heading: LibraryListHeading` (`Mark(ServiceMark)` or `Title(String)`), as design D5 states. Update the module doc. In `library_list_header/tests.rs`, change `the_title_line_paints_the_title_and_the_trigger` to a `Mark` heading (mark hook present, no title hook, trigger present), and add one test: a `Title` heading paints its text and no mark. Verify: `cargo nextest run -p mbv-gui library_list_header`.
- [ ] 5.2 Pass the D5 heading from the seven callers: `library_list.rs`, `library_emby_list.rs`, `library_shows_list.rs`, `library_audiobooks_list.rs`, `library_podcasts_list.rs`, `library_feeds_list.rs` (`ServiceMark::feeds(frame.platform)`, `Rss` while no frame), and `library_home_list.rs` (`Title("Home")`). Remove the Shows `list_title` computation and its `title_attr`. Change the tests that read `[data-library-list-title]` in `library_feeds_list/tests.rs`, `library_audiobooks_list/tests.rs`, `library_emby_list/tests.rs`, and `library_podcasts_list/tests.rs` to read the `data-library-list-mark` value (`rss`, `audiobookshelf`, `emby`, `audiobookshelf`). Leave the Home test as it is. Verify: `cargo nextest run -p mbv-gui`.

## 6. Docs

- [ ] 6.1 Add the terms *Service mark* (the brand mark that names a destination's Service, or the Feed platform of the checked subscriptions, in the GUI Library list header) and *Feed platform* (the known service that serves a FeedSubscription, read from its URL alone) to `CONTEXT.md`, under Services and under Feeds, each with an *Avoid* line. Update `docs/architecture/gui-client.md` near line 440 (the brand marks now also live in `window/service_mark.rs`) and near line 950 (the header's title line paints a Service mark or, for Home, a title). Verify: `qmd query "Service mark"` finds the new `CONTEXT.md` entry.

## 7. Integration

- [ ] 7.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-feed -p mbv -p mbv-gui`. Verify: all three pass with no warnings.
