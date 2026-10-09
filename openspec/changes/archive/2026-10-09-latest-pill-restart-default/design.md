# Design

## D1: The snapshot records the persisted pill scope, not the live pill

Each owner's `launch_snapshot` reports the scope a restart should open, not the pill the user left active:

- Emby library owners (`EmbyLibraryContent`): `Latest` when the owner paints a selector row (`selector_mode` is `Letters` or `FeedGroups`), `None` otherwise. A teardown inside a folder (no pills) restores the browse position without a pill, as today.
- TV (`TvContent`): `None`. The TV mode and letter pill are session memory; the restart default comes from the count-dependent resolve (D5).
- Podcast (`PodcastContent`): `Latest` always.
- Feeds (`FeedsContent`): `Latest` when subscriptions exist, `None` otherwise.
- Grouped Music (`MusicContent`): the selected group, unchanged — the user explicitly keeps Music pill persistence.
- Book (`BookContent`): the selected bucket, unchanged — same user decision.
- Home: its fixed Continue scope, unchanged.

Every owner stops reporting a selected item: the `item` half of the snapshot tuple is always `None`. The tuple shape stays decodable so snapshots written by older versions still parse, but a saved item SHALL NOT restore — restoration always lands on the first selectable row of the restored scope (`tui-launch-state`: "A saved library item, letter range, show, watched filter, or other non-scope pill in a snapshot written by an older version SHALL NOT restore"); the next orderly exit rewrites the file without an item.

## D2: Restore honors only the persisted scope

`launch_selector`/`reanchor_launch_state` accept the scope the new snapshots carry and treat everything else as legacy:

- Emby: only `EmbySelectorKey::Latest` applies (`EmbyLatest` when the owner has pills and is not already in Latest mode). Legacy `Letter`/`Group`/`Unfiltered` selectors decode but apply nothing, leaving the destination default.
- TV: `launch_selector` always returns `None`; the mode default arrives through the load path.
- Podcast: `launch_selector` emits `AudiobookshelfLatest` only when the snapshot says `Latest` and the pill differs; `reanchor_launch_state` sets `Latest` and ignores legacy filter/show keys. Every podcast library starts on `Latest` on every restart, whether or not it was the selected destination at orderly exit (`audiobookshelf-podcast-library-ui/spec.md:123`) — not just the restored tab.
- Feeds: `reanchor_launch_state` selects Latest whenever subscriptions exist; legacy filter/group keys resolve to Latest.

`launch_selector` for Emby also guards `selector_mode == None` so a pill-less owner never gets forced into Latest mode.

## D3: The legacy position document cannot resurrect a pill

`library_position_state` is loaded from disk at construction and never written (existing rule). Its `letter_filter_index`, `tv_content_mode`, and `feed_selected_group` fields are cleared on every loaded entry, so an old file restores positions (cursor/scroll/levels) but never a pill. Mid-session entries written by `save_default_library_position` keep their pill fields in memory — that is session memory and tab re-entry still restores it.

## D4: Mid-session behavior unchanged

Owner pill state survives tab switches (D2 retention), the in-memory position map still restores browse positions on re-entry, and Latest acknowledgement/marker behavior is untouched.

## D5: TV restart default stays count-dependent

With the saved mode stripped, `resolve_tv_content_mode(total, None)` gives `Latest` above `LIBRARY_PILL_THRESHOLD` and `All` at or below — the existing count-dependent default, now applied on every restart.

## D6: F4 toggles the Playlists sidebar like F2/F3

`Command::OpenPlaylists` (`src/app/dispatch/action.rs:270`) and `ShellRequest::OpenPlaylists` route through `request_sidebar_toggle`/`toggle_sidebar`, so a second F4 dismisses the panel the first F4 mounted — parity with the F2/F3 sidebar rule. The playlist load spawn moved into `mount_sidebar` beside the Sessions loads; `open_playlists_panel` and its hand-rolled exclusivity dismissals are gone (`mount_sidebar` already unmounts the other sidebars).

## D7: The selected icon-only Home tab paints no block runs and keeps the active colour

When the Continue tab shows its icon label (`is_home_icon_title`, `crates/mbv-ui-model/src/ui_util.rs:16`), the selected tab keeps the normal ACCENT_ACTIVE (Iris) text colour and paints no eighth-block runs (`crates/mbv-render/src/components/chrome_tabs.rs:186`). 2026-10-09 user rule (`ca8fdab33` drops the runs; `ba48238c8` restores Iris instead of Mauve).

## D8: A shuffle-sourced queue paints no source pill

`queue_source_status_label` returns `None` for `QueueSource::Shuffle` (`src/app/state/projection/chrome_status.rs:466`): the SHUFFLE label was never requested (2026-10-09 user rule; `4d2f4d38b`). ALBUM/SERIES/REMOTE Q/collection labels are unchanged.

## Non-goal

The first-load default pill for a library that is not the exit tab is unchanged (large non-TV Emby libraries still open on their first letter range when first loaded mid-session) — except podcast libraries, which start on `Latest` on every restart whether or not they were the exit tab. This change governs what persists across restart, not the built-in first-load default.
