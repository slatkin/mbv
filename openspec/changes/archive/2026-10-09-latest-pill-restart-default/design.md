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

Every owner stops reporting a selected item: the `item` half of the snapshot tuple is always `None`. The tuple shape and the `reanchor_launch_state` item handling stay, so snapshots written by older versions still decode (the Home precedent); the next orderly exit rewrites the file without an item.

## D2: Restore honors only the persisted scope

`launch_selector`/`reanchor_launch_state` accept the scope the new snapshots carry and treat everything else as legacy:

- Emby: only `EmbySelectorKey::Latest` applies (`EmbyLatest` when the owner has pills and is not already in Latest mode). Legacy `Letter`/`Group`/`Unfiltered` selectors decode but apply nothing, leaving the destination default.
- TV: `launch_selector` always returns `None`; the mode default arrives through the load path.
- Podcast: `launch_selector` emits `AudiobookshelfLatest` only when the snapshot says `Latest` and the pill differs; `reanchor_launch_state` sets `Latest` and ignores legacy filter/show keys.
- Feeds: `reanchor_launch_state` selects Latest whenever subscriptions exist; legacy filter/group keys resolve to Latest.

`launch_selector` for Emby also guards `selector_mode == None` so a pill-less owner never gets forced into Latest mode.

## D3: The legacy position document cannot resurrect a pill

`library_position_state` is loaded from disk at construction and never written (existing rule). Its `letter_filter_index`, `tv_content_mode`, and `feed_selected_group` fields are cleared on every loaded entry, so an old file restores positions (cursor/scroll/levels) but never a pill. Mid-session entries written by `save_default_library_position` keep their pill fields in memory — that is session memory and tab re-entry still restores it.

## D4: Mid-session behavior unchanged

Owner pill state survives tab switches (D2 retention), the in-memory position map still restores browse positions on re-entry, and Latest acknowledgement/marker behavior is untouched.

## D5: TV restart default stays count-dependent

With the saved mode stripped, `resolve_tv_content_mode(total, None)` gives `Latest` above `LIBRARY_PILL_THRESHOLD` and `All` at or below — the existing count-dependent default, now applied on every restart.

## Non-goal

The first-load default pill for a library that is not the exit tab is unchanged (large non-TV Emby libraries still open on their first letter range when first loaded mid-session). This change governs what persists across restart, not the built-in first-load default.
