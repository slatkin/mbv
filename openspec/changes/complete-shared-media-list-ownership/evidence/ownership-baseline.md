# Ownership baseline

Change: `complete-shared-media-list-ownership`, Section 1 (tasks 1.1–1.3)
Baseline: `90310dca` (`refactor/finish-canonical-media-list-ownership`)

## Accepted vocabulary

The accepted shared-owner name is **`MediaList<Target>`**. `MediaList` does not
collide with a defined CONTEXT term or an `_Avoid_` entry; the existing
`generic list` Avoid entry rejects a vague description, not this named
provider-neutral owner. `WideMediaList` and `InlineMediaBrowser` remain the
one-column Variant names. **Grid presentation** is the accepted name for the
shared-owner Variant planned for the existing non-hero two-column catalog
arrangement; it is not a new provider or destination. The existing `two-column list`
Avoid entry remains respected.

## Production owner and test map

The map covers every flow named by the delta requirement, each applicable
presentation, and provider-workspace row flow. Existing owner files are listed
first; characterization/component tests follow.

| In-scope flow | Production owner(s) | Wide | Inline / Normal / Narrow | Grid / workspace | Existing characterization and interaction tests |
|---|---|---|---|---|---|
| Queue slots | `src/app/components/queue.rs`; `src/app/render/components/queue.rs` | `WideMediaList<QueueSlotId>` fixed rows in all panel modes | N/A (fixed queue presentation) | N/A | `queue_component_tests.rs`, `queue_drag_component_tests.rs`, `render/tests_queue.rs`, `queue_title_characterization_tests.rs` |
| Home rows | `src/app/components/home.rs`; `src/app/render/components/home.rs`, `home_latest_row.rs` | Wide fixed rail | Inline hero / selected-row replacement | N/A | `home_component_tests.rs`; `render/tests_home_characterization.rs`, `tests_home_inline.rs` |
| Generic Emby catalog rows | `src/app/components/browser/mod.rs`, `browser/paint.rs`, `browser/state.rs` (destination cursor/scroll mirrors to remove), plus shared mechanics `src/app/components/media_list/{mod,wide,inline,anchor,grouping}.rs` | Wide hero catalog | Normal/Narrow inline catalog | Existing non-hero two-column catalog policy | `browser_component_tests.rs`, `browser_mouse_tests.rs`, `browser_inline_search_tests.rs`; `render/tests_library_characterization.rs`, `tests_non_music.rs`, `tests_conformance_matrix.rs` |
| Movies | Same Browser owners plus `BrowserKind::Movies` paths | Wide | Inline | N/A | `browser_component_tests.rs`, `browser_mouse_tests.rs`; `render/tests_library_characterization.rs` |
| Emby homevideos feed view | Same Browser owners plus homevideo grouping paths | Wide grouped feed-like catalog | Inline grouped feed-like catalog | N/A | `browser_component_tests.rs`, `browser_mouse_tests.rs`; `tests_narrow_browse_migration.rs`, `tests_library_characterization.rs` |
| Grouped Music albums | `src/app/components/music_workspace.rs`; `render/components/music_workspace.rs` | Wide album rail | Inline album browser | N/A | `music_workspace_actions_tests.rs`, `music_workspace_cursor_tests.rs`; `render/tests_music_characterization.rs`, `tests_music_groups.rs`, `tests_music_wide.rs`, `tests_music_wide_reanchor_characterization.rs` |
| Grouped Music tracks | `src/app/components/music_workspace.rs`; `render/components/music_workspace.rs` | Wide track table | Provider workspace / filtered track view | N/A | `music_workspace_actions_tests.rs`, `music_workspace_cursor_tests.rs`; `render/tests_music_wide.rs` |
| TV series | `src/app/components/tv_workspace/mod.rs`, `navigation.rs`; `render/components/tv_workspace.rs` | Wide series rail | Browser/Normal series presentation | N/A | `tv_workspace_component_tests.rs`; `render/tests_library_characterization.rs`, `tests_non_music.rs` |
| TV episodes | `tv_workspace/mod.rs`, `navigation.rs`; `render/components/tv_workspace.rs` | Wide episode workspace | Normal episode/browser path | N/A | `tv_workspace_component_tests.rs`; `tests_library_characterization.rs`, `tests_non_music.rs` |
| Feeds entries | `src/app/components/feeds.rs`; `render/components/feeds.rs` | Wide one-column Feeds list | Normal/Narrow inline replacement | N/A | `feeds_component_tests.rs`; `render/tests_feeds.rs`, `tests_feeds.rs`, `tests_narrow_browse_migration.rs` |
| Audiobookshelf Podcast shows | `src/app/components/audiobookshelf_podcast.rs`; `render/components/audiobookshelf_podcast.rs` | Wide show rail | Inline show replacement | N/A | `audiobookshelf_podcast_component_tests.rs`; `render/tests_audiobookshelf_podcasts.rs` |
| Audiobookshelf Podcast filtered episodes | `src/app/components/audiobookshelf_podcast.rs`; `render/components/audiobookshelf_podcast.rs` | Wide episode workspace | Normal/Narrow filtered episode workspace | N/A | `audiobookshelf_podcast_component_tests.rs`; `render/tests_audiobookshelf_podcasts.rs` |
| Audiobookshelf Books | `src/app/components/audiobookshelf_book.rs`; `render/components/audiobookshelf_book.rs` | Wide book rail | Inline book replacement | N/A | `audiobookshelf_book_component_tests.rs`; `render/tests_audiobookshelf_books.rs` |
| Audiobookshelf chapter/audio-part rows | `src/app/components/audiobookshelf_book.rs`; `render/components/audiobookshelf_book.rs` | Wide chapter workspace | Normal/Narrow chapter workspace | N/A | `audiobookshelf_book_component_tests.rs`; `render/tests_audiobookshelf_books.rs` |

**Completeness verification:** every flow in the delta's first ADDED
requirement is present above: Queue, Home, generic Emby (including non-hero
Grid), Movies, Emby homevideos, grouped Music albums/tracks, TV series/episodes,
Feeds, Audiobookshelf Podcast shows/filtered episodes, and Audiobookshelf Books
and chapter/audio-part rows. Wide, Inline/Normal/Narrow, Grid where applicable,
and provider-workspace paths are explicitly accounted for; no listed flow or
breakpoint is omitted.

## Characterization and mutation checks

The baseline already contains focused characterization for all four required
path classes, so no duplicate Rust test was added in Section 1. Focused tests
were run unchanged. For each check below, a disposable working-tree mutation
was made, the named test was run and failed, the mutation was reverted, and the
same test was rerun and passed. No mutated path is committed.

| Test | Disposable disconnect | Mutated result | Restored baseline |
|---|---|---|---|
| `media_list::tests::wide_component_retains_only_completed_current_frame_facts` | disconnected Wide painter retained-fact publication | FAIL | PASS |
| `media_list::tests::wide_list_maps_display_rows_to_selectable_indices_and_viewport` | disconnected `Item` target resolution | FAIL | PASS |
| `home_component_tests::home_down_moves_the_component_cursor_without_app_state` | disconnected local movement delegation | FAIL | PASS |
| `media_list::tests::resolve_point::wide_resolves_against_a_scrolled_viewport` | disconnected retained row-hit target resolution | FAIL | PASS |
| `media_list::tests::inline_component_retains_detail_and_resolves_from_the_current_view` | disconnected Inline painter/detail fact publication | FAIL | PASS |
| `browser_component_tests::browser_mouse_uses_the_painted_two_column_cell_for_left_and_right_clicks` | disconnected Grid cell target resolution | FAIL | PASS |


The unchanged focused suite also exercises Inline replacement, two-column Grid
cell targeting, provider-workspace movement, retained hits, stable targets,
and Wide↔Inline anchors through the tests listed in the map above.
