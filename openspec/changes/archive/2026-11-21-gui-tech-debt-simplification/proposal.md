# Proposal

## Why

The GUI Client (`crates/mbv-gui`) grew one surface and one destination at a time. Recent fixes show the cost: one behaviour must be changed in four to six parallel files, for example `8c6c53d38` (the row menu on every Library row), `1508227bb` (Service marks), `ac9c4b52c` (delineated rows), and `3a11e845f` (per-destination sort, about 40 window files). The architecture review of 2026-11-21 found shallow modules and copied rituals in the window, the bridge, and the core thread. A simplification audit of the same day found the same shapes in the row memos, the Heroes, the Settings panes, and the list wiring, and its findings joined as the later workstreams. This change is one tech-debt pass for efficiency and simplification. It does not change behaviour. It is deliberately multifaceted: more workstreams can join it from later code reviews.

## What Changes

The change is a set of workstreams. Each one keeps all observable behaviour, layout, and paint the same.

- **Reported region set.** One region-set module in `window/layout` owns the layout keys, the epoch subscription, the report, and the retract for each surface. Eleven components stop repeating a report effect of about 30 lines. Marks run only in the surface-owner's render; a child component — the queue header, the playback stage, the four popovers — takes the mark closures its owner produces as props, so the parents drop their extra keys, their row-handle lists, and their truncation code. `row_handle.rs` goes away. The six test copies of the last-report reader become one helper.
- **Dialog frame.** One dialog frame holds the scrim, the card, the button row, and the button regions of the confirm dialog and the daemon-lost dialog.
- **Lettered-list module.** One generic list component, one row model, and one set of filter and scroll state methods serve Emby, Audiobooks, Podcasts, and Feeds, through one trait with four adapters. Home and Shows keep their own modules. The duplicated list and keys tests move onto the shared module.
- **Episode helpers.** The episode helpers move out of `shows_workspace`, so the Feeds Hero and the shared row part stop importing Shows logic.
- **Queue row rule.** One `active_slot` rule replaces the two copies in the bridge. The chooser's row-height alias goes away.
- **Section batch.** The window applies the section batch by destructuring it, so a section that the window does not apply fails the build.
- **Pointer routing.** Pointer routing moves out of `core_thread.rs` into its own child module.
- **Rows memo.** One generic row memo replaces the seven one-slot memo types beside the Library row models; the revision-keyed hit test and its `peek` rule live once.
- **Hero actions.** The destination Heroes' Play, Play next, and Add to queue buttons route through the same command methods the list keys use; the five private action enums go, and the Emby Hero's inline selection guard becomes the shared helper.
- **Settings pane scaffold.** One scaffold owns the Settings section panes' shared prologue and grid, and one setup-form builder replaces the two hand-built Service setup forms.
- **Shared list wiring.** The list-column wiring the lettered module owns serves Music, Home, and Shows too; their row models, headers, and notices stay their own.
- **Small rules.** One art-source rule on `GuiArtRef`; one armed-prefix badge part; the Shows view dropdown owns its open fact; the section store writes `connected` only when it flips; the icons generate from one macro.
- **Duplicated tests.** Tests that re-test one contract per destination or per surface move onto the deeper module. Only cases with a different outcome stay. The change adds no new tests.

Considered and dropped (see design.md): a generic projection pipeline in `src/app/gui_library`, because its common rules are already shared; one popover module, because after the region set only differing looks are left; a data key spec, because `PanelKeys` is already that seam; one owner for the queue column state, because the risk to behaviour is too high for the gain; collapsing the `bridge/frame/library` row and Hero frames behind one shared row-facts struct, because this change changes no wire types — the four identical `LocationRow` impls in `window/library/status.rs` note it for a later change.

Out of scope: the queue row numbering bug and the possible hit-map inversion on remount (both on the scratchpad), and any change of look.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a refactor with no change to behaviour, so the change sets `skip_specs: true`. The work keeps the code in line with existing `gui-client` requirements: "Library lists and Workspace tables paint through shared parts", "Every Hero is one frame with the largest unit as its title", "GUI pointer input resolves through semantic targets", and "The window updates only the sections that changed".

## Impact

- `crates/mbv-gui/src/window/layout.rs` and the eleven components that report regions: `queue_panel`, `queue_playback_panel`, `track_chips`, `queue_target_control`, `confirm_overlay`, `daemon_lost_overlay`, `folded_queue`, `compact_strip`, `collapsed_rail`, `queue_footer`, and `attach_screen`. Also the four popovers (`queue_row_menu`, `queue_chooser_menu`, the target chooser, and the track popover), `row_handle.rs`, the child components whose handle signals become mark closures (`queue_header`, `queue_playback_panel/art_stage`, `queue_playback_panel/scrubber`), and `QueueRowMenu`'s five non-reporting callers (`library_panel`, `library_hero`, `playlists_panel`, `settings_pick_button`, `settings_feeds`), which drop their dead `row_slot` signals.
- `crates/mbv-gui/src/window/components/library_{emby,audiobooks,podcasts,feeds}_list*` and `crates/mbv-gui/src/window/library/{emby,audiobooks,podcasts,feeds}{,_list,_keys}*`.
- `crates/mbv-gui/src/window/library/shows_workspace.rs` and the four callers of its episode helpers.
- `crates/mbv-gui/src/bridge/frame/{queue,playback}.rs`, `crates/mbv-gui/src/window/sections.rs`, and `crates/mbv-gui/src/core_thread.rs`.
- `crates/mbv-gui/src/window/library/row_memo.rs` and the six row-model modules whose memo types it replaces; `crates/mbv-gui/src/window/components/library_{emby,audiobooks,podcasts,feeds,home}_hero.rs`; `crates/mbv-gui/src/window/library/state.rs` (the memo fields, and the Shows view dropdown's own open fact); `crates/mbv-gui/src/window/app.rs`, `crates/mbv-gui/src/window/components/library_panel.rs`, `crates/mbv-gui/src/window/components/library_shows_hero.rs`, and `crates/mbv-gui/src/window/library/keys.rs` (every path that closes the Shows view dropdown today closes it through its own fact).
- `crates/mbv-gui/src/window/components/settings_{screen,rows,keys,sessions,services,feeds,library}.rs`; `crates/mbv-gui/src/window/components/{library_list,library_home_list,library_shows_list,library_virtual_list}.rs`; the `connected` flip in `crates/mbv-gui/src/window/sections.rs`; `crates/mbv-gui/src/art.rs`; `crates/mbv-gui/src/window/components/{queue_footer,library_status_bar,settings_status_bar}.rs`; `crates/mbv-gui/src/window/icons.rs`, `input.rs`, and `library/scroll.rs`.
- Tests next to all of these modules. The number of test lines goes down.
- Docs: `docs/architecture/gui-client.md` (layout reports and the hit map; the Library panel's shared list and Hero pieces; the one list-wiring scaffold every list shares).
- The change adds no dependencies and changes no wire format. The ctrl protocol, persistence, the TUI Client, `src/app`, and the specs do not change.
- Overlap: the open change `gui-playlists-in-queue-column` edits `queue_playback_panel.rs`, `panel.rs`, `app.rs`, and `library/keys.rs`. Implementation of this change starts after that change merges.
