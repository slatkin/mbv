# Proposal

## Why

The GUI Client (`crates/mbv-gui`) grew one surface and one destination at a time. Recent fixes show the cost: one behaviour must be changed in four to six parallel files, for example `8c6c53d38` (the row menu on every Library row), `1508227bb` (Service marks), `ac9c4b52c` (delineated rows), and `3a11e845f` (per-destination sort, about 40 window files). The architecture review of 2026-11-21 found shallow modules and copied rituals in the window, the bridge, and the core thread. This change is one tech-debt pass for efficiency and simplification. It does not change behaviour. It is deliberately multifaceted: more workstreams can join it from later code reviews.

## What Changes

The change is a set of workstreams. Each one keeps all observable behaviour, layout, and paint the same.

- **Reported region set.** One region-set module in `window/layout` owns the layout keys, the epoch subscription, the report, and the retract for each surface. Eleven components stop repeating a report effect of about 30 lines. The four popovers mark their rows with their parent's set, so the parents drop their extra keys, their row-handle lists, and their truncation code. `row_handle.rs` goes away. The six test copies of the last-report reader become one helper.
- **Dialog frame.** One dialog frame holds the scrim, the card, the button row, and the button regions of the confirm dialog and the daemon-lost dialog.
- **Lettered-list module.** One generic list component, one row model, and one set of filter and scroll state methods serve Emby, Audiobooks, Podcasts, and Feeds, through one trait with four adapters. Home and Shows keep their own modules. The duplicated list and keys tests move onto the shared module.
- **Episode helpers.** The episode helpers move out of `shows_workspace`, so the Feeds Hero and the shared row part stop importing Shows logic.
- **Queue row rule.** One `active_slot` rule replaces the two copies in the bridge. The chooser's row-height alias goes away.
- **Section batch.** The window applies the section batch by destructuring it, so a section that the window does not apply fails the build.
- **Pointer routing.** Pointer routing moves out of `core_thread.rs` into its own child module.
- **Duplicated tests.** Tests that re-test one contract per destination or per surface move onto the deeper module. Only cases with a different outcome stay. The change adds no new tests.

Considered and dropped (see design.md): a generic projection pipeline in `src/app/gui_library`, because its common rules are already shared; one popover module, because after the region set only differing looks are left; a data key spec, because `PanelKeys` is already that seam; one owner for the queue column state, because the risk to behaviour is too high for the gain.

Out of scope: the queue row numbering bug and the possible hit-map inversion on remount (both on the scratchpad), and any change of look.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a refactor with no change to behaviour, so the change sets `skip_specs: true`. The work keeps the code in line with existing `gui-client` requirements: "Library lists and Workspace tables paint through shared parts", "Every Hero is one frame with the largest unit as its title", "GUI pointer input resolves through semantic targets", and "The window updates only the sections that changed".

## Impact

- `crates/mbv-gui/src/window/layout.rs` and the eleven components that report regions: `queue_panel`, `queue_playback_panel`, `track_chips`, `queue_target_control`, `confirm_overlay`, `daemon_lost_overlay`, `folded_queue`, `compact_strip`, `collapsed_rail`, `queue_footer`, and `attach_screen`. Also the four popovers (`queue_row_menu`, `queue_chooser_menu`, the target chooser, and the track popover) and `row_handle.rs`.
- `crates/mbv-gui/src/window/components/library_{emby,audiobooks,podcasts,feeds}_list*` and `crates/mbv-gui/src/window/library/{emby,audiobooks,podcasts,feeds}{,_list,_keys}*`.
- `crates/mbv-gui/src/window/library/shows_workspace.rs` and the four callers of its episode helpers.
- `crates/mbv-gui/src/bridge/frame/{queue,playback}.rs`, `crates/mbv-gui/src/window/sections.rs`, and `crates/mbv-gui/src/core_thread.rs`.
- Tests next to all of these modules. The number of test lines goes down.
- Docs: `docs/architecture/gui-client.md` (layout reports and the hit map; the Library panel's shared list and Hero pieces).
- The change adds no dependencies and changes no wire format. The ctrl protocol, persistence, the TUI Client, `src/app`, and the specs do not change.
- Overlap: the open change `gui-playlists-in-queue-column` edits `queue_playback_panel.rs`, `panel.rs`, `app.rs`, and `library/keys.rs`. Implementation of this change starts after that change merges.
