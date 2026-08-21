# Unmigrated surfaces

One row per independently rendered surface. A row is ticked in the same PR that
migrates it (tasks.md step 5). This list may shrink; it may not grow except when a
genuinely new surface is added to the app.

`Coverage` is buffer-test coverage as of 2026-08-21 and sets how much
characterization work commit 1 of that surface's migration carries.

| # | Surface | Files | Coverage | Issue | Done |
|---|---|---|---|---|---|
| 1 | Emby standard library | `list.rs`, `list_rows.rs`, `list_letter_groups.rs`, `list_plain.rs`, `movies_wide.rs`, `tv_wide.rs`, `detail.rs`, `detail_series.rs`, `detail_series_view.rs`, `hero.rs`, `hero_left.rs` | good (`list_tests.rs` 758, `movies_wide_tests.rs` 359, `tv_wide_tests.rs`) | #564 | [ ] |
| 2 | Emby home-video | `home_video.rs` | some | #565 | [ ] |
| 3 | Emby music and album | `music.rs`, `music_wide.rs`, `music_wide_browser.rs`, `album.rs`, `album_art.rs`, `album_cursor.rs`, `album_detail.rs`, `album_plan.rs`, `album_rows.rs` | some (`tests_music_groups.rs`, `tests_album_listing.rs`) | #566 | [ ] |
| 4 | Home screen | `home.rs`, `home_hero.rs`, `home_latest_row.rs`, `home_list_rows.rs`, `home_pills.rs`, `home_feed.rs` | thin (`home_tests.rs` 498B, `tests_home_inline.rs`) | #567 | [ ] |
| 5 | Feeds screen | `feeds.rs` | thin (`tests_feeds.rs` 1.8K) | #568 | [ ] |
| 6 | Audiobookshelf podcast | `audiobookshelf.rs` | some (`tests_audiobookshelf_podcasts.rs`) | #569 | [ ] |
| 7 | Audiobookshelf book | `audiobookshelf_books.rs`, `audiobookshelf_book_browser.rs` | some (`tests_audiobookshelf_books.rs`) | #570 | [ ] |
| 8 | Search sidebar | `search_sidebar.rs` | **none** | #571 | [ ] |
| 9 | Settings sidebar | `overlays/settings.rs` | **none** | #572 | [ ] |
| 10 | Playlists panel and dialog | `overlays/playlists.rs` | **none** | #573 | [ ] |
| 11 | Sessions sidebar | `overlays/sessions.rs` | **none** | #574 | [ ] |
| 12 | Feed-management popup | `overlays/feeds_manage.rs` | **none** | #575 | [ ] |
| 13 | Help sidebar | `overlays/help.rs` | some (4 inline) | #576 | [ ] |
| 14 | Remote-reanchor popup | `overlays/remote_reanchor.rs` | **none** | #577 | [ ] |
| 15 | Library-routes popup | `overlays/library_routes.rs` | some (7 inline) | #578 | [ ] |
| 16 | Context menu | `overlays/context_menu.rs` | **none** | #579 | [ ] |
| 17 | Daemon-lost modal | `overlays/daemon_lost_modal.rs` | **none** | #580 | [ ] |
| 18 | Confirm modal | `overlays/confirm_modal.rs` | **none** | #581 | [ ] |
| 19 | Multiselect popup | `overlays/multiselect.rs` | **none** | #582 | [ ] |

Not surfaces — shared already, handled by step 1's move rather than a migration PR:
`card.rs`, `widgets.rs`, `indicators.rs`, `pills.rs`, `chrome*.rs`, `queue.rs`,
`sort_filter.rs`, `visualizer.rs`, `overlays/backdrop.rs`, `overlays/modal_frame.rs`.
