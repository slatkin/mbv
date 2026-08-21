# Unmigrated surfaces

One row per independently rendered surface. A row is ticked in the same PR that
migrates it (tasks.md step 5). This list may shrink; it may not grow except when a
genuinely new surface is added to the app.

`Coverage` is buffer-test coverage as of 2026-08-21 and sets how much
characterization work commit 1 of that surface's migration carries.

| # | Surface | Files | Coverage | Issue | Done |
|---|---|---|---|---|---|
| 1 | Emby standard library | `list.rs`, `list_rows.rs`, `list_letter_groups.rs`, `list_plain.rs`, `movies_wide.rs`, `tv_wide.rs`, `detail.rs`, `detail_series.rs`, `detail_series_view.rs`, `hero.rs`, `hero_left.rs` | characterized (`tests_library_characterization.rs`, `list_tests.rs`, `movies_wide_tests.rs`, `tv_wide_tests.rs`, `detail_tests.rs`) | #564 | [x] |
| 2 | Emby home-video | `home_video.rs` | characterized (`home_video_tests.rs`) | #565 | [x] |
| 3 | Emby music and album | `music.rs`, `music_wide.rs`, `music_wide_browser.rs`, `album.rs`, `album_art.rs`, `album_cursor.rs`, `album_detail.rs`, `album_plan.rs`, `album_rows.rs` | characterized (`tests_music_characterization.rs`, `tests_music_groups.rs`, `tests_album_listing.rs`) | #566 | [x] |
| 4 | Home screen | `home.rs`, `home_hero.rs`, `home_latest_row.rs`, `home_list_rows.rs`, `home_pills.rs`, `home_feed.rs` | characterized (`tests_home_characterization.rs`, `tests_home_inline.rs`, `home_tests.rs`) | #567 | [x] |
| 5 | Feeds screen | `feeds.rs` | characterized (`tests_feeds.rs`) | #568 | [x] |
| 6 | Audiobookshelf podcast | `audiobookshelf.rs` | characterized (`tests_audiobookshelf_podcasts.rs`) | #569 | [x] |
| 7 | Audiobookshelf book | `audiobookshelf_books.rs`, `audiobookshelf_book_browser.rs` | characterized (`tests_audiobookshelf_books.rs`) | #570 | [x] |
| 8 | Search sidebar | `search_sidebar.rs` | characterized (`tests_search_sidebar.rs`) | #571 | [x] |
| 9 | Settings sidebar | `overlays/settings.rs` | characterized (`tests_settings.rs`) | #572 | [x] |
| 10 | Playlists panel and dialog | `overlays/playlists.rs` | characterized (`tests_playlists.rs`) | #573 | [x] |
| 11 | Sessions sidebar | `overlays/sessions.rs` | characterized (`tests_sessions.rs`) | #574 | [x] |
| 12 | Feed-management popup | `overlays/feeds_manage.rs` | characterized (`tests_feeds_manage_popup.rs`) | #575 | [x] |
| 13 | Help sidebar | `overlays/help.rs` | characterized (`tests_help.rs`) | #576 | [x] |
| 14 | Remote-reanchor popup | `overlays/remote_reanchor.rs` | characterized (`tests_remote_reanchor.rs`) | #577 | [x] |
| 15 | Library-routes popup | `overlays/library_routes.rs` | characterized (`tests_library_routes_popup.rs`) | #578 | [x] |
| 16 | Context menu | `overlays/context_menu.rs` | characterized (`tests_context_menu.rs`) | #579 | [x] |
| 17 | Daemon-lost modal | `overlays/daemon_lost_modal.rs` | characterized (`tests_daemon_lost_modal.rs`) | #580 | [x] |
| 18 | Confirm modal | `overlays/confirm_modal.rs` | characterized (`tests_confirm_modal.rs`) | #581 | [x] |
| 19 | Multiselect popup | `overlays/multiselect.rs` | characterized (`tests_multiselect.rs`) | #582 | [x] |

Not surfaces — shared already, handled by step 1's move rather than a migration PR:
`card.rs`, `widgets.rs`, `indicators.rs`, `pills.rs`, `chrome*.rs`, `queue.rs`,
`sort_filter.rs`, `visualizer.rs`, `overlays/backdrop.rs`, `overlays/modal_frame.rs`.

## Phase-1 migration notes

- Emby home-video uses the Movie overview/detail hero additional-content style.
  Its painting remains in the shared `components/home_video.rs` vocabulary; no
  bespoke component or new structural variant is required.
- Feeds use the feed-entry metadata hero style without an image.
- Audiobookshelf podcasts use the show hero and episode-workspace style;
  Audiobookshelf books use the book hero and chapter-workspace style.
- Search, settings, playlists, sessions, feed-management, help, remote-reanchor,
  library-routes, context-menu, and daemon-lost surfaces use existing sidebar,
  popup, or modal vocabulary; none needs a bespoke component or new structural
  variant.
- Confirm and multiselect overlays reuse the modal-frame vocabulary; their
  rendering is owned by dedicated components and their state transitions remain
  in the existing app-side module.

## Phase-2 migration notes

- Emby standard-library movies use the shared Movie overview/detail hero; TV
  uses the TV season/pill and episode workspace. Wide library panes share the
  `arrangements/library.rs` geometry and existing hero/list components.
- Music uses the Music track-list workspace in `arrangements/music.rs`; grouped
  albums use the shared album rows/detail components in both inline and wide
  presentations.
- Home uses the existing Emby hero card and generic provider-specific hero data;
  its narrow presentation is Inline hero and its wide presentation is
  Hero-on-left. No bespoke component or new structural variant was required.
