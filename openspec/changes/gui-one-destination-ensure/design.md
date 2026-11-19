# Design

## Context

See proposal.md for the defect. Today the GUI frame path (`crates/mbv-gui/src/sections/library.rs`, `build` and the `*_parts` functions) calls one ensure per destination: `ensure_gui_music_selection`, `ensure_gui_shows_workspace`, `ensure_gui_emby_selection`, `ensure_gui_abs_selection`, and `ensure_gui_podcast_selection` (which `build` runs before the parts), and nothing for Feeds and Home. The destination open step (`open_gui_active_destination` in `src/app/gui_library.rs`) is a second place where loads start. It has an arm for Shows, Emby, Audiobooks, Podcasts, and Feeds, and `_ => {}` for the rest.

Observed state of the root load per destination:

| Destination | Root load starts from | Selection / dependent loads start from |
|---|---|---|
| Music | the shared startup fetch of the groups catalog | `ensure_gui_music_selection` (frame) |
| Shows | `ensure_gui_shows_workspace` (frame; the #1021 fix) and `gui_tv_open` | `ensure_gui_shows_workspace` |
| Emby library | `ensure_gui_emby_selection` (frame) and `gui_emby_open` | `ensure_gui_emby_selection` (feed-view libraries) |
| Audiobooks | `gui_abs_open` only | `ensure_gui_abs_selection` (frame) |
| Podcasts | `gui_podcast_open` only | `ensure_gui_podcast_selection` (frame) |
| Feeds | the shared startup fetch | none |

Audiobooks and Podcasts have the same gap Shows had: a restore at launch skips the open command. The `GuiCollectionSite` trait (`collection_command.rs`) models collection commands (toggle, All, order, restore), not loads, and it covers only four destinations. It is not the base for this change.

## Goals / Non-Goals

**Goals:**
- One entry point on `App` that the frame path, the open command, and the router's destination steps all use.
- One order of steps for every destination, enforced by a trait whose three steps have default no-op bodies, with an exhaustive match on the destination kind so a new destination must name its site.
- No change to what any single fetch does, or to its guards.

**Non-Goals:**
- No change to the TUI's load paths. The Shared shell loaders (`audiobookshelf_refresh`, `audiobookshelf_book_refresh`, `activate_library_position`, `apply_tv_letter_mode`) stay as they are.
- No merge of `GuiCollectionSite` with this trait.
- No new fetch, cache, or event type.

## Decisions

**D1. A new private trait `GuiDestinationLoads` in `src/app/gui_library/ensure.rs`.** It has three methods with default no-op bodies, `ensure_root`, `ensure_selection`, and `ensure_dependents`, each taking `&mut App`. The first two return a step outcome, and the last step returns nothing. One driver, `App::ensure_gui_destination(kind)`, takes a `GuiDestinationKind`, matches it exhaustively to a small site value (`MusicLoads { lib_idx }`, `ShowsLoads { lib_idx }`, and so on), and runs the three methods in order. The match has no wildcard arm (AGENTS.md, interactive architecture rule 5).
*Alternative:* keep five ensure functions and add a convention. Rejected: a convention is what failed. *Alternative:* extend `GuiCollectionSite`. Rejected: it does not cover Shows, Audiobooks, or Home, and it mixes command rejection with loading.

**D2. The driver is the only caller.** `build` in `sections/library.rs` makes one call, `app.ensure_gui_active_destination()`, before it builds the parts, and removes the per-destination ensures from `music_parts`, `shows_parts`, `emby_parts`, `audiobooks_parts`, and the Podcasts call at the top of `build`. `open_gui_active_destination` keeps only the user-action work (D4), then calls the driver. The keyboard router's destination steps already go through `open_gui_library_position`.

**D3. The steps keep their current idempotent guards.** Each step is the existing code moved behind the trait:
- Music: root is a no-op (the startup fetch owns the groups catalog); selection is the body of `ensure_gui_music_selection`; dependents is a no-op.
- Shows: root is `gui_shows_root_needs_load` then the root load; selection is the default series' detail load; dependents is the selected season's episodes load. A step that starts a load ends the pass, as `ensure_gui_shows_workspace` returns after `fetch_series_detail` today, so the order stays strict.
- Emby library: root is `fetch_gui_emby_root_items` for a plain library; selection is `ensure_gui_emby_feed_fetch` for a feed-view library; dependents is a no-op.
- Audiobooks: root is the catalog refresh `gui_abs_open` runs today (same `unloaded` guard, including `loading_pages` and `replacement`); selection is the effective pick's detail load.
- Podcasts: root is the show-list refresh `gui_podcast_open` runs today (same guard); selection is the show scope commit; dependents is All's shelf fetch and the per-show fan-out.
- Feeds and Home: every step is a documented no-op.

**D4. The open command keeps its user-action work, then runs the driver.** `gui_tv_open` keeps forcing the All view mode (the GUI's one Shows view). `gui_emby_open` keeps clearing a failed root mark, because the existing rule says a failed list retries on an open, not on a repaint. Their root loads move into the driver. `gui_abs_open`, `gui_podcast_open`, and `gui_feeds_open` keep their ignored-command log for a `lib_idx` that names no matching library and then run the driver. Wire commands (`absOpen`, `podcastOpen`, `tvOpen`, `embyOpen`, `feedsOpen`) do not change.

**D5. The active-tab guard moves into the driver.** Today `ensure_gui_abs_selection`, `ensure_gui_podcast_selection`, and the open steps each test `self.tab.audiobookshelf_index() == Some(lib_idx)`, because the shell loaders read the active tab's browse state. The driver runs only for `gui_active_destination()`, which is the active tab, but a collection command can carry a stale frame's destination, so the driver tests the active tab and the `gui_*_library` guard once for the Audiobooks and Podcasts kinds, before it runs their steps. The two wrapper entries and the `debug_assert`s are gone.

**D6. Public ensure functions become `pub(in crate::app::gui_library)` trait steps.** `ensure_gui_music_selection`, `ensure_gui_emby_selection`, `ensure_gui_podcast_selection`, `ensure_gui_abs_selection`, and `ensure_gui_shows_workspace` stop being `pub` on `App`. Tests that call them call `ensure_gui_destination` for the same destination. The `mbv-gui` crate stops importing them.

**D7. No dependency, version, renderer, or backend choice.** The change adds none.

## Risks / Trade-offs

- [The Audiobooks or Podcasts root load now starts from the frame, which could start a second catalog chain beside the startup chain.] → The steps keep the open step's existing `unloaded` guard (empty list, zero total, no `loading_pages`, no `replacement`). A test seeds each in-flight shape and asserts no second request.
- [A repaint retries a failed root load.] → Emby's failed mark stays honoured by the frame; only the open command clears it. Audiobooks and Podcasts root loads end in a non-empty or total-zero state only on success, so a failure that leaves the state unloaded would repeat each frame. The task for these steps checks the failure shape in the shell loaders and, if a failure leaves the state unloaded, records a failed mark in the same style as `GuiEmbyRootItems::Failed`. This is part of the work, not an open question.
- [Strict ordering delays a dependent load by one frame.] → Acceptable; the one-fetch-at-a-time rule already does this for Music and Emby feed groups.
- [A moved guard changes behavior silently.] → The migrated tests keep their assertions and only change the call site, so a changed outcome fails a test.

## Migration Plan

Internal refactor with one behavior addition (Audiobooks and Podcasts catalog from the frame). No data migration. Roll back by reverting the commits.
