# Design

## Context

See proposal.md (Why) for the motivation. The facts that shape the approach:

- **Region reports.** Eleven components in `crates/mbv-gui/src/window/components/` each hold one or more `LayoutKey`s (`layout.rs:226`). Each holds one `Signal<Option<Event<MountedData>>>` per control, or a `Vec` of row handles. Each runs one effect that reads its sections and the `LayoutEpoch`, spawns one task that calls `element_region` (`layout.rs:430`) for each handle, and then calls `report_layout`. Each retracts in `use_drop`. `queue_panel.rs` holds three keys. `queue_target_control.rs`, `queue_playback_panel.rs`, and `track_chips.rs` hold two. The extra keys exist only so a popover's rows merge after the content under them. The reporter merges in first-push order, and the last push wins in the hit map. A popover's key retracts while the popover is closed, so each open pushes it at the end again. One task reports the base key and then the popover key, in that order. `track_chips.rs`, `confirm_overlay.rs`, and `daemon_lost_overlay.rs` trigger the report through `use_reactive` on props. The others read signals in `use_effect`.
- **Popovers.** The four popovers (`QueueRowMenu`, `QueueChooserMenu`, `TargetChooserPopover`, `TrackPopoverList`) are presentation-only. Each takes a `row_slot: Signal<Vec<Option<Event<MountedData>>>>` that it fills with `record_row_handle` (`row_handle.rs:12`). The parent truncates that list and reports it under its second key. Their card styles, anchors, and row heights all differ (40, 34, 36, and 16 px rows; three different card token sets). `record_row_handle` has no callers other than these four. `QueueRowMenu` also serves five menus the window owns — the Library item menu (`library_panel.rs`), the Hero menu (`library_hero.rs`), the Playlists menu (`playlists_panel.rs`), the Settings pick (`settings_pick_button.rs`), and the Feed kind menu (`settings_feeds.rs`) — whose rows never reported: they pass `row_slot` only because the prop is required, nothing reads those signals, and their entries' presses run `on_select`, never the hit map.
- **Dialogs.** `confirm_overlay.rs` and `daemon_lost_overlay.rs` repeat the same scrim, card, button row, button fill, `HashMap` of button handles, and region report. Only the padding, gap, maximum width, and button text size differ (19/14/398/14 against 16/12/420/12).
- **Lettered lists.** Emby, Audiobooks, Podcasts, and Feeds repeat each layer:
  - The list component: `components/library_{emby,audiobooks,podcasts,feeds}_list.rs`, 210 to 247 lines.
  - The row model: `library/{emby,audiobooks,podcasts,feeds}_list.rs`, about 145 lines each, over the shared `build_lettered_rows`.
  - The window state methods in `library/{emby,audiobooks,podcasts,feeds}.rs`: set and close filter, press, play, queue, step scroll, jump to letter, measure, and scroll.

  The real differences are data: the attribute word, the heading key prefix, the art scope, the pointer list, the header mark, label, and placeholder, the notice texts, and the row painter (Emby and Audiobooks build `LibraryItemRow` themselves; Podcasts and Feeds use `paint_item_row`). There are three differences in behaviour. Audiobooks, Podcasts, and Emby commands carry `lib_idx`, but Feeds commands do not. Only Emby has play-next. Feeds has `select_feeds_row_id` for its Hero table. Home is different in kind: it has headings and spacers, no rail, no sort, no failed line, opaque Continue and Recent ids, and its own key router (`keys.rs:176-266`).
- **Keys.** The keys already sit behind a real seam: the `PanelKeys` trait in `keys_shared.rs`, with default methods, has four adapters, and `route_panel_list_key` is shared. Only the four keys test suites repeat the same harness.
- **Projections.** The Client-core projections already share their common rules: `GuiListSort::rail_letter` and `is_name_sort` in `list_sort.rs`, and `picked_or_first` in `entry_list.rs`. What stays per destination (catalog walk, collection merge, comparators, the Emby feed-group branch) differs in substance.
- **Heroes.** The Hero frame is already shared (`LibraryHeroFrame`, `hero_layout`). Destination heroes differ in actions and child tables. `is_played_episode` and `episode_progress` live in `library/shows_workspace.rs:115-128` and have callers in `library_feeds_hero.rs`, `library_item_row.rs`, `library_shows_hero.rs`, and `library_shows_list.rs`.
- **Queue rows.** `active_slot` is byte-identical in `bridge/frame/queue.rs:236` and `bridge/frame/playback.rs:331`. `queue_chooser_menu.rs:53` aliases `ITEM_HEIGHT_PX = ROW_HEIGHT_PX`.
- **Sections.** The core `take_batch` is already a struct literal, so a new batch field without a `take` fails to compile. The window `apply_batch` (`window/sections.rs:191`) reads `batch.<field>` one at a time, so a new field that it does not read compiles.
- **Core thread.** `core_thread.rs` has 798 lines. Pointer routing is `route_pointer` (482-640), `route_row_drag_gesture` (643-665), and `queue_row_at` (438-440).
- **Dependencies.** The stack is pinned at Dioxus Native `=0.8.0-alpha.3`. `CopyValue` reaches every component through `dioxus_native::prelude` (it re-exports `dioxus_signals::*`). An `onmounted` handler can be a block that evaluates to a closure (`queue_target_control.rs:254`).

## Goals / Non-Goals

**Goals:**
- Every workstream keeps the painted output, the reported layout (regions, their order, and the keys' merge order), the commands sent to the core, and the key routing exactly as they are.
- Each workstream merges on its own, and the order of the workstreams is free, except that the dialog frame and the popover part of the region set come after the region set itself.
- The structure leaves room for more workstreams to join this change.

**Non-Goals:**
- No change of look. Style constants, card tokens, and row heights stay as they are.
- No change to the reporter's merge rule, and no explicit paint layers.
- No change to the wire types in `bridge/frame/`, or to the ctrl protocol.
- No new tests. Only duplicated tests move or go.

## Decisions

### D1. One region set per component instance

New child module `crates/mbv-gui/src/window/layout/region_set.rs`. Its interface:

- `use_region_set(overlays: usize, watch: impl FnMut() + 'static) -> RegionSet` serves signal-driven surfaces. The set runs one `use_effect` that calls `watch` (the surface's current section reads) and reads the `LayoutEpoch`.
- `use_region_set_for(overlays, deps)` serves the three props-driven surfaces. It takes the same dependency tuple that the surface gives `use_reactive` today, and also reads the epoch.
- `RegionSet::mark(target) -> impl FnMut(Event<MountedData>)` serves the base contribution.
- `RegionSet::mark_over(i, target)` serves overlay contribution `i` (`i < overlays`).
- `RegionSet::mark_list(list: ListId, whole_rows: fn(f32) -> usize)` serves the one measured list (the queue). It reports `ListRows { list, whole_rows: whole_rows(height) }`.
- `RegionSet` is `Clone`. The `mark*` calls return `EventHandler`s, so a mark's closure can pass to a child component as a prop.

How the set works:

- **At render.** The hook call at the top of the component body marks a new render and clears the "painted this render" list. Each `mark*` call during render appends its target to that list, and the closure it returns stores the element's handle under that target. Both live in one `CopyValue`, which is not reactive, so a mark never triggers a render and no `Rc` is necessary. Marks run in the surface-owner's own render — inline in its `rsx` or in `let`s before it — in today's report order. A child component never calls `mark`: Dioxus memoizes a child whose props compare equal, and a child's render runs after its parent's, so a mark inside a child could be skipped while its element still paints — the open popover's key would retract — or land after the parent's own marks. For an element a child owns, the owner calls `mark` and passes the closure to the child as a prop — the queue header's four controls, the playback stage's and scrubber's controls, and the popovers' rows all work this way. A mark keeps today's paint-vs-report gates: a control that claims no pointer today is marked only while it acts — the queue header's close control is marked only while `close_disabled` is false.
- **The report task.** One effect spawns one task. The task resolves the painted targets of the base, in mark order, and reports them under the base key. Then it does the same for each overlay key in index order. A base with no marks still reports its empty contribution, as `folded_queue` does today. An overlay with no marks retracts its key, as each popover's key does today when it closes, so a popover that opens again pushes at the end, as now. The task also drops stored handles whose targets were not painted in the latest render.
- **Retract.** `use_drop` retracts every key of the set.

`mark`, `mark_over`, and `mark_list` are new vocabulary beside the multi-selection *Mark* (`CONTEXT.md`): the region-set sense records there with the change (task 2.9).

Why this preserves behaviour: the trigger signals, the task count (one), the key creation order (hook order), the first-push timing, and the mark order are each equal to today's. Before a migration, each surface's current region order is compared with its `rsx` order. Where overlapping regions (the queue row and its grip and actions, the art stage and the transport controls) are pushed in an order other than paint order today, the surface marks in today's order.

Alternatives considered:
- An explicit paint layer, with the merge ordered by layer. It was rejected because it changes the merge rule. The possible inversion on remount stays a separate scratchpad item.
- A set inside each popover component, with its own effect. It was rejected because the child's and the parent's tasks could then push in either order the first time.
- A key reserved at hook time. It was rejected because the popover would then merge under surfaces that are created after its parent.
- Measuring every render. It was rejected because playhead re-renders would add `element_region` calls.

### D2. Popovers take the owner's mark closures

Each of the four popovers replaces its `row_slot` prop with `row_marks: Vec<EventHandler<MountedData>>` — the mark closures the owner produced by calling `mark_over` at the popover's position in its `rsx`, one per painted row in row order. The owner computes each row's target exactly as it maps index to target today. Each parent drops its second and third keys, its row-handle `Vec`s, its truncation code, and its popover report block. The look, the anchor, and the row paint of each popover stay in its own module.

`QueueRowMenu` also serves the five menus whose rows never reported (the Context's popover note), so its `row_slot` stays beside `row_marks` as an `Option` (`#[props(into)]`, so the unmigrated call sites compile): a row uses its `row_marks` entry when the vector reaches it and records into `row_slot` otherwise. `row_slot` and `row_handle.rs` go when the last caller flips.

Alternative considered: one popover module with the look as parameters. It was rejected because, once D1 is in place, only chrome strings and row painters would stay, and they all differ, so the module would be shallow.

### D3. One dialog frame

New `components/dialog_frame.rs`. It holds the scrim, the card on `MENU_CARD_STYLE`, the button row with `BUTTON_FILL` and `button_color`, and a region set for the buttons. Its props are the four sizes (padding, gap, maximum width, button text size), the buttons, the region target for each button index, and the body as children. `confirm_overlay.rs` and `daemon_lost_overlay.rs` keep only their body and their sizes. `BUTTON_FILL` and `button_color` move into `dialog_frame.rs`. `menu_card.rs` stays, because five modules share its constant.

### D4. One lettered-list module for Emby, Audiobooks, Podcasts, and Feeds

- **The trait.** A new trait `LetteredDestination` in `window/library/lettered.rs` has four zero-sized adapters (`EmbyList`, `AudiobooksList`, `PodcastsList`, `FeedsList`). It holds:
  - An associated `Row` type, the destination's existing row frame.
  - The data: the attribute word, the heading key, the art scope, the pointer list, the header facts, and the notice texts.
  - The accessors for the list and detail parts, its `ListUiState`, and its scroll store.
  - The row painter.
  - The command methods `select`, `play`, `queue`, and `play_next`, where `play_next` defaults to "no play-next".
- **What becomes generic.**
  - One generic component `LibraryLetteredList<D>` in `components/library_lettered_list.rs` replaces the four list components.
  - One generic row model (`LetteredItemRow<R>`, `build_rows::<D>`) replaces the four `library/*_list.rs` row modules.
  - One set of generic `LibraryState` methods for filter, scroll, letter jump, and measure replaces the four sets.
- **What stays.** Feeds' `select_feeds_row_id` and `feeds_hero_row_at` stay Feeds methods. `AudiobooksUiState` becomes an alias of `ListUiState`, as the other three are. The `PanelKeys` adapters stay and call the generic state methods.
- **Out of scope here.** Home and Shows keep their own modules and already paint through the shared row parts that the `gui-client` spec requires.

Alternatives considered:
- A data descriptor of function pointers passed as props. It was rejected because props must be `PartialEq`, and comparing function pointers is unreliable (rustc lint `unpredictable_function_pointer_comparisons`). A zero-sized trait parameter needs no comparison. Four adapters make the seam real.
- Home in the module. It was rejected because Home's row model, header, notices, and key router differ in behaviour, and moving Home in would need behaviour flags.
- A key spec as data. It was rejected because `PanelKeys` already gives that seam.
- Collapsing `parts.rs`. It was rejected because each narrow function returns a distinct type, so a generic version needs a trait and saves nothing.

### D5. No generic projection pipeline

The proposed `FlatSource` pipeline in `src/app/gui_library` is dropped. The common rules are already in `list_sort.rs` and `entry_list.rs`. The rest differs per destination, so a generic function would add a wide trait over little shared logic.

### D6. Episode helpers in one module

`is_played_episode` and `episode_progress` move unchanged to a new `window/library/episode.rs`. The four callers import from there, so `library_feeds_hero.rs` and `library_item_row.rs` stop depending on `shows_workspace`.

### D7. One active-slot rule and no row-height alias

`bridge/frame/queue.rs` keeps `active_slot` as `pub(crate)`. `playback.rs` deletes its copy and imports this one. `queue_chooser_menu.rs` reads `ROW_HEIGHT_PX` and `ROW_GAP_PX` directly. `geometry.rs` and `row_cells.rs` stay as they are: after D1 the row regions are marks in the row's `rsx`, and nothing is left to merge.

### D8. The window applies the batch by destructuring it

`apply_batch` starts with `let GuiSectionBatch { attach, theme, … } = batch;`, which names every field and has no `..`. A new batch field that it does not apply then fails the build as an unused binding (warnings are denied in CI). The rejected alternatives were a generic window slot (the existing `write` and `write_shared` are already generic) and a registry macro or type erasure (harder to read, and the section collect runs on every tick).

### D9. Pointer routing in its own child module

`route_pointer`, `route_row_drag_gesture`, `queue_row_at`, and `gesture_name` move unchanged into a new `core_thread/pointer.rs` as an `impl CoreThread` block. This follows the existing `tick_host.rs` and `overlays.rs` split. No test moves.

### D10. Duplicated tests

- **Lettered lists.** In `library_{emby,audiobooks,podcasts,feeds}_list/tests.rs`, the common contracts (painted rows and order, the rows-in-view art report, press selects, double-click plays, selected fill, loading and empty lines, the header, the right-click menu) become one suite in `library_lettered_list/tests.rs`. A contract whose command or text differs per destination becomes a named `#[case]` table over the four adapters. The cases that only one destination has (Emby's feed-group mode, Audiobooks' disabled rail letters, Podcasts' and Feeds' date gutter) stay.
- **Keys.** The four keys suites share one harness in `window/library/keys_shared/tests.rs`.
- **Region readers.** The six copies of `last_report` and `last_reported_targets` become one `#[cfg(test)]` helper in `window/layout/region_set.rs`, next to the set.
- **Heroes.** The three per-destination tests that assert only the frame's band order (Audiobooks, Podcasts, Feeds) become one frame test.

No test is new in kind, and the test line count goes down.

### D11. One rows memo

`window/library/row_memo.rs` holds one generic `RowsMemo<Row, Facts: PartialEq>`: one entry (the `libraryList` section revision, the facts, and the `Arc<[Row]>`), one `rows()` that keeps the `peek`-never-`read` hit test, and one `Debug` impl. The seven memo types become typed uses of it, and `LibraryState`'s memo fields keep their names. The lettered four migrate beside the generic row model D4 builds.

Alternative considered: a trait the per-list memos implement. It was rejected because the entry is one struct and the trait would only restate the hit test.

### D12. Hero actions through the destination's commands

After D4's command methods, each Hero's Play, Play next, and Add to queue buttons call the destination's command methods through `LibraryState`, passing the frame's selected row as the item id. The five private `Action` enums and `act` functions go, and the Emby Hero's inline selection guard becomes the shared `selected_with_hero`. The `IMDb` link and the chapter and episode Workspace plays stay in their Heroes: they name no destination command method.

Alternative considered: keeping the per-Hero `act`s. It was rejected because each would hold the same command mapping the destination's command methods already hold.

### D13. One Settings pane scaffold

`settings_screen.rs` owns the panes' shared prologue: one read that consumes the two contexts, guards the `settings_section_view`, builds the `onpress` over `press_settings_entry`, and yields the cursor id, beside one two-column grid constant. The six panes keep their content matches and their bodies. One setup-form builder over a field list — the label, the secret flag, and the update closure — replaces `emby_form` and `audiobookshelf_form`; `form_actions` stays the actions' one home.

### D14. The shared list wiring serves every list

The wiring the lettered component absorbs for the four lettered lists — the measure, scroll, and rows-in-view callbacks, the `library_virtual_list` call, the header block, and the notices block — becomes one scaffold beside `library_virtual_list.rs`, and Music, Home, and Shows call it with their own row painters and header facts. Their row models, headers, and notices stay per destination, exactly as D4 keeps their modules: this shares the wiring, not the list.

Alternative considered: leaving the three hand-wired. It was rejected because the wiring is the same shape in every list, and the next destination would copy it a seventh time.

### D15. Small rules

- `GuiArtRef::src()` in `art.rs` replaces the two `art_src` copies.
- One `PrefixBadge` part (`window/components/prefix_badge.rs`) replaces the three badge paints; each surface keeps its own data attribute.
- The Shows view dropdown owns its open fact: `LibraryUiState` gains `shows_view_open` with its own toggle and close, and every path that closes the popover through the shared fact today closes it through its own: the root's every-press (`window/app.rs`), the Library panel's inside-press (`press_in_panel` in `window/components/library_panel.rs`), the Shows Hero's inside-press (`window/components/library_shows_hero.rs`), and Esc — `route_open_field_key` (`window/library/keys.rs`) gains a `shows_view_open` arm that closes on Esc and takes no typed key, as its seasons arm does. The two shows-view keys tests (`window/library/keys/tests/collections.rs`) follow the new fact, so they keep guarding the real popover. The collections' open fact names the collection dropdowns alone again, so every window-owned dropdown owns its open fact, as the seasons dropdown already does.
- `apply_batch` writes `connected` only on the false-to-true flip, so a batch the root does not read re-renders only its own readers. No paint, reported region, or command changes; only the root's re-render count goes down.
- The `KeyRoute` doc's repeated sentences go, and `LibraryScrolls::Debug` names every store.

### D16. The icons macro

One declarative macro in `icons.rs` generates each `#[component]` from its name, its aliases, and its path children. `icon_shell` and the painted output stay identical, so the class-list tests stay green with no edit. This workstream rides last: it is the least valuable of the set, and it only rewrites a frozen vendored port.

## Risks / Trade-offs

- [Region order changes for overlapping regions] → Each migration compares the effect's push order with the `rsx` mark order before the edit and keeps today's order. The existing region tests (`queue_panel/tests/regions.rs`, `track_chips/tests.rs`, `queue_footer/tests.rs`, and the dialog and attach suites) must pass without edits to their expected targets.
- [A popover child skips or lags its parent's render] → No mark runs inside a child (D1): Dioxus memoizes a child whose props compare equal, and a child's render runs after its parent's, so a mark inside a child could be skipped while its element still paints — the open popover's key would retract — or land after the parent's own marks. The owner calls `mark_over` at the popover's rsx position instead, so a painted popover's rows always record and a closed popover's always retract, as today.
- [The generic list moves a paint difference] → Each adapter's row painter is the destination's current paint code, moved unchanged. Tests that assert painted text and attributes stay green with no change to their expectations.
- [This overlaps the open change `gui-playlists-in-queue-column`] → Implementation of this change starts after that change merges (it edits `queue_playback_panel.rs`, `panel.rs`, `app.rs`, and `library/keys.rs`).
- [The generic memo moves a hit test] → The hit test and the `peek` rule move unchanged into the one entry; every list suite that asserts rows stays green with no edit.
- [A Hero button changes its command] → Each button sends the command its Hero sends today for the same frame; the Hero suites stay green with no edit to their expected commands.
- [The `connected` flip changes rendering] → No paint, reported region, or command changes; only the root's re-render count on batches the root does not read goes down. The window suites stay green.
- [800-line limit] → No file in this change grows. `queue_panel.rs`, `core_thread.rs`, `settings_services.rs`, and `icons.rs` shrink.

## Migration Plan

This is a refactor in `crates/mbv-gui` only. Each task group is a commit series that can be reverted on its own. Nothing persists, nothing goes on the wire, and no config changes, so no migration or rollback step is necessary beyond a revert.
