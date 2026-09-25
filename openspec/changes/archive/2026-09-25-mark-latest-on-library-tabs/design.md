# Design

## Context

Today each destination computes its marker only inside its own content push, and only while its tab is active:

| Destination | Push site | Marker input |
|---|---|---|
| Emby Movies/generic | `shell/emby_library_content.rs` | `tv_latest_snapshots[lib_id].has_new_content` |
| Emby TV | `shell/tv_workspace.rs` | `tv_latest_snapshots[lib_id].has_new_content` |
| Audiobookshelf podcast | `shell/audiobookshelf_podcast.rs` | `audiobookshelf_shelf_cache[lib_id]` in launch window |
| Feeds | `shell/feeds.rs` | `feed_tab.all_entries` `pub_date_secs` in launch window |

Each result is ANDed with `!acknowledged_home_latest_sources.contains(source)`. The pill painter draws `•` in `palette::ACCENT_ACTIVE` (Iris). `sync_tab_panel` (`shell/chrome_panels.rs`) runs every tick and projects titles in position order: Home, `app.libs`, `app.audiobookshelf_libraries`, then Feeds when subscribed.

All four inputs are already stored per destination, so the marker can be worked out for inactive tabs without new fetches.

## Goals / Non-Goals

**Goals:** a tab marker that always agrees with the pill marker; the least code churn.

**Non-Goals:** removing the pill marker; changing when markers are acknowledged; fetching Latest for destinations that don't load it today (for example a TV library whose mode is not Latest has no snapshot, so it stays unmarked, the same as its pill today).

## Decisions

**D1: One shared marker predicate on `Model`.** Add `fn destination_latest_marker(&self, source: &DestinationLatestSource) -> bool` (for example in `shell/home_content.rs` beside the acknowledgement helpers). It returns the in-window check for that source's stored items AND not acknowledged. The four push sites replace their inline `has_new && !acknowledged` with a call to it, and the tab sync calls it too. That way the tab and pill markers can't drift apart. The alternative, a second inline computation in `sync_tab_panel`, would copy the Feeds and podcast window logic and could drift. Each push site keeps its existing "record acknowledgement when Latest is selected" block unchanged, and that block runs before the marker is read.

**D2: Per-tab markers are a parallel `Vec<bool>`.** `TabPanel::set_content` takes `markers: Vec<bool>` parallel to `titles`, and `TabBarModel` gains `markers: &[bool]`. `sync_tab_panel` builds it in the same chain order as the titles: `false` for Home, `Emby(lib.id)` for each Emby lib (Music/playlists libraries yield `false` because they have no snapshot), `Audiobookshelf(lib.id)` for each ABS library (book libraries have no shelf cache entry, so `false`), and `Feeds`. This mirrors the existing pill `markers` shape. A per-tab struct would add a type for a single flag.

**D3: No width change.** Titles paint as `"  {NAME}  "` (unselected) and `"▐ {NAME}  "` (selected). A marked tab replaces the first trailing space with `•` in `palette::ACCENT_ACTIVE`, so the painted width stays the same. `tab_title_widths` and `visible_tab_range` (shared with keyboard tab cycling) stay untouched.

## Risks / Trade-offs

- [The tab marker is only as fresh as the stored snapshot] → The same freshness as the pill has today; refreshes already update the stores, and the tick-driven `sync_tab_panel` picks them up.
- [Emby snapshot `has_new_content` is already cleared by `record_home_latest_acknowledgement`, and the Feeds and podcast checks aren't] → D1 applies the acknowledgement check uniformly, so every source behaves the same.
