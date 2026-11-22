# Design: 1-decouple-client-core

## Context

See proposal.md (Why). This is change 1 of 3 (`1-decouple-client-core`, `2-split-tui-gui-clients`, `3-add-owner-program`). The decision numbers (D2, D4, ...) are the numbers of the original single-change plan, kept in all three designs so that cross-references stay true. This design holds D2, part of D3, D4, D4a, D5, D6, D6a, D6b, D7, and D13. `2-split-tui-gui-clients` holds D1, the rest of D3, D4b, D4c, D10 (part), D11, and D12. `3-add-owner-program` holds D8, D9, and the Owner parts of D3 and D10.

The current state that shapes the approach:

- The root package `mbv` is a library and the `mbv` binary. Its `src/app/` holds the shared `App` (182 fields), dispatch (~23.5k lines), state (~11k lines), the keyboard router (`src/app/input/`, ~5k lines), the TUI shell (`src/app/shell/`, ~14.7k lines), and the GUI-only modules `gui_library`, `gui_settings`, `gui_playlists`, `gui_client_state` (~14k lines).
- `crates/mbv-gui` depends on the root `mbv` package. It uses `App`, `ClientTickHost`, the `Gui*Command` types, and the `test`-feature seams (`stub_app_with_queue_and_cmd_rx`).
- The GUI-only modules are `impl App` blocks. They read 42 non-GUI `App` fields directly, own 26 `gui_*` fields on `App`, and add GUI variants to the shared event enum (`GuiGroupAlbums`, `GuiEmbyRootItems`, `GuiShowsNew`).
- No GUI module names the TUI shell. Outside `shell/` and `input/`, these non-test files use ratatui or crossterm: `state/projection/{card,chrome_status,tv_wide,visualizer}.rs`, `infra/terminal.rs`, `infra/image_fetch.rs`, `infra/image_fetch/protocol.rs`, and `test_seams.rs` (`crossterm_key_to_neutral`, also tuirealm and `mbv-components`). Inside `input/`, `input.rs` builds a ratatui `Rect` from `terminal_width` and `terminal_height` for the tab-strip width. One shared file uses the shell: `dispatch/run_loop/teardown.rs` (`shell::Model`).
- Shared code and GUI-only code also import items from the TUI crates `mbv-render` and `mbv-components`: 27 non-test shared files and 13 GUI module files. Some items are toolkit-free shared logic that lives in a TUI crate (hero content builders, album plans, sort keys, book rows). Others are TUI terminal geometry that shared state reads (`MINI_VIEW_THRESHOLD`, `chrome_geometry`, `wide_library_panes`). So the GUI uses TUI code today through these crates. D13 places each item.
- `App::gui_client` is the one Client-kind fact. It gates more than the Mini rule: the terminal-width Mini view and the hidden empty queue column (`client_arranges_panels`), the visualizer and visual-slot hide setters (`visual_slot_controls`), the shown-queue rules in `state/named_queues.rs` and `state/queue_link.rs` (including the closed-queue fallback in `adopt_named_queue_list`), and the router gates `GuiClient` and `GuiQueueTarget` in `input/key_policy.rs`. D6a replaces it.
- The prefs file is one JSON object that `App::save_prefs` reads, changes, and writes back. It holds keys that only one Client reads (`gui_player_auto_hide`, `visual_slot_hidden`, `queue_column_width`, `list_pane_width`).
- Shared signatures and shared state carry terminal cells: the context-menu anchor `Option<(u16, u16)>` in `build_queue_selection_menu_at`, the `x`/`y` cells of the `open_context_menu_*` calls, and `ContextMenuAnchor::Pointer { x, y }` (D6).
- `mbv-theme` has an optional `ratatui` feature. `mbv-images` depends on `ratatui` and `ratatui-image` (crossterm feature).
- `ClientTickHost` already gives each Client its own drain hooks for the shared tick. The TUI implements it on the shell `Model`, the GUI on its `CoreThread`.
- No core signature takes GUI state. Core code calls about 25 GUI-module methods directly as `App` methods (D4a lists them).
- A first attempt at the whole split failed at one row (94 library and 697 test errors in one step), partly because it threaded GUI state through core functions. D4a answers that.

## Goals / Non-Goals

**Goals:**

- Shared code (`src/app/` outside `shell/` and the GUI modules) names no ratatui, crossterm, tuirealm, `mbv-render`, or `mbv-components` in non-test files.
- `App` holds no GUI state, the core calls no GUI method, and the core names no Client.
- `mbv-theme`, `mbv-images`, and `mbv-ui-model` have no terminal library in their dependency graphs.
- Each Client keeps its present behaviour. The TUI and the GUI attach to one Owner at the same time.

**Non-Goals:**

- No new crate for the core or a Client (`2-split-tui-gui-clients`).
- No `pub` access surface for `App` beyond what a step needs. The GUI modules and the shell stay inside `crate::app` and keep their direct access; `2-split-tui-gui-clients` builds the surface (D4b).
- No Owner program and no packaging change (`3-add-owner-program`).
- No new user feature, key, screen, or command. No change to the ctrl protocol or the wire schema. No change to `mbvd`.
- No merge of the TUI Library and the GUI Library panel. They present different surfaces.

## Decisions

### D2. Placement rule

For each function, type, field, and test:

1. Both Clients reach it (directly, or through the shared router and `Command` dispatch) → the core (`src/app/` outside `shell/` and the GUI modules).
2. Only one Client reaches it, and it is presentation or Client-local state (layout, terminal cells, window state, a surface that Client alone presents) → that Client's code (the shell `Model`, or `GuiCore`).
3. Only one Client reaches it, but it is Owner, Service, queue, or playback behaviour → the core, as a call. It is not UI code, and the other Client can call it later without a move.
4. A type both Clients need but that has no `App` dependency → the lowest shared leaf crate that fits (`mbv-ui-model`, `mbv-ui-msg`, `mbv-surfaces`).

Where a GUI-only and a TUI-only entry point wrap one shared function, the shared function stays in the core and each wrapper moves to its Client. Where two Clients have two copies of one rule, the copy in the core stays and the other is deleted.

### D3 (part). TUI code moves under the shell

| Today | Goes to |
|---|---|
| `src/app/state/projection/{card,chrome_status,tv_wide,visualizer}.rs` | `src/app/shell/`, as shell functions over `App` reads; a part that builds no ratatui value stays in `state/` |
| `src/app/shell/draw.rs`: `impl App` (`compute_frame_layout`, `compute_chrome_geometry`, `queue_panel_placement`, `compose_root_frame`) | shell free functions over `App` reads: `effective_panel_mode`, `effective_panel_focus`, `effective_playback_state`, `queue_column_width`. The terminal size and `layout` (`AppLayout`) it writes become shell `Model` fields (D4, D13 Rule B). The core keeps only the neutral left panel area that the TUI pushes (D13 Rule B) |
| `src/app/infra/terminal.rs`, `src/app/infra/image_fetch/protocol.rs`, ratatui parts of `infra/image_fetch.rs` | `src/app/shell/` |
| `src/app/dispatch/run_loop/teardown.rs` (names `shell::Model`) | the `Model` part goes to `src/app/shell/`; the shared teardown stays in `dispatch` as a call over `App` |
| crossterm key and mouse translation in `src/app/input/` | `src/app/shell/`; the router takes only neutral events |
| `src/app/input.rs`: `tab_scroll`, `ensure_tab_visible`, `visible_tab_range`, and the tab-strip width from `terminal_width`, `terminal_height`, a ratatui `Rect`, and `chrome_geometry` | `src/app/shell/`; the core keeps only a tab-reveal request (D6b) |
| `src/app/test_seams.rs`: `crossterm_key_to_neutral` and every other seam that names crossterm, tuirealm, or `mbv-components` | `src/app/shell/`, with the shell-driving tests that use it; the toolkit-free seams stay |
| `src/app/tests.rs`: ratatui and crossterm helpers (`TestBackend`, `Terminal`, the crossterm `confirm_replace_queue`) | a shell test-support module; the toolkit-free helpers stay |

### D4. Each Client wraps `App` by composition

`App` stays the shared state. Each Client owns it:

- TUI: the shell `Model` already owns `App`. It also takes the TUI-only state that leaves `App`: `terminal_width`, `terminal_height`, the narrow-terminal Mini view state, `layout` (D13 Rule B), `tab_scroll` (D6b), and every other field that only the shell reads or writes.
- GUI: `GuiCore { app: App, gui: GuiClientState }` owns `App` and all GUI state. `GuiClientState` holds the 24 `gui_*` fields that remain after D6a (of 26: `gui_client` is replaced by `ClientCapabilities`, and `gui_shown_entry` stays in the core as `shown_entry`), plus `panel_collapsed` and `queue_folded`, which only GUI code reads or writes. The `impl App` blocks of the GUI modules become `impl GuiCore` blocks. `GuiCore` is defined in `src/gui.rs`, in the root package but outside `crate::app`, so that `2-split-tui-gui-clients` does not need to move it inside the package (its D4c puts the GUI modules beside it). The GUI's `CoreThread` holds `GuiCore` and keeps its `ClientTickHost` implementation.
- Prefs: the file format and every key name stay the same. The core keeps `load_prefs` and writes only the keys that the core owns. It gets one `pub fn save_client_pref(key: &str, value: serde_json::Value)` with the same read, change, and write. Each Client reads its own keys from `load_prefs` at start and writes them through `save_client_pref` when they change: the GUI owns `gui_player_auto_hide`, and the TUI owns every key whose field D2 moves to the shell `Model`. A key whose field stays in the core stays a core key.

No `App` field becomes `pub` in this change.

Alternatives considered:

- `App<X: ClientExt>` with a Client-specific slot. Rejected: it makes ~70k lines generic, and every signature changes.
- `Box<dyn Any>` client slot with downcasts. Rejected: it hides the boundary from the compiler.
- An extension trait on `App` in each Client crate. Rejected: it needs `pub` fields once the Clients are crates.

### D4a. The core calls no GUI function

**Today (HEAD).** No core signature takes GUI state. The GUI modules are `impl App` blocks, and core code calls their methods directly, as plain `App` methods. The GUI state is the `gui_*` fields on `App`. Measured at plan time, core non-test code outside the GUI modules calls these GUI-module methods:

| Core caller | GUI-module methods it calls | Disposition |
|---|---|---|
| `dispatch/action.rs` (`App::dispatch` arms) | `toggle_gui_queue_chooser`, `toggle_gui_target_chooser`, `toggle_gui_track_popover`, `toggle_panel_collapse`, `toggle_queue_fold` | GUI half (a) |
| `dispatch/action.rs` | `activate_queue_filter`, `request_close_shown_queue`, `request_new_gui_queue`, `request_rename_shown_queue` | Core (b) |
| `shell/playback.rs` (`App::handle_playback_request`, shared playback-request handling) | `open_gui_player_menu`, `select_gui_player_menu_entry`, `select_gui_track_popover_track`, `show_gui_queue_chooser_entry`, `toggle_panel_collapse`, `toggle_queue_fold` | GUI half (a) |
| `shell/playback.rs` | `activate_queue_filter` | Core (b) |
| `shell/messages/navigation.rs` (TUI) | `select_chooser_target` | Core (b): both Clients call it |
| `dispatch/session/player_event.rs`, `client_drains.rs` | `report_named_queue_rejection`, `take_new_queue_answer` | Core (b) |
| `input/confirm_keys.rs` | `send_named_queue_close` | Core (b) |
| `dispatch/library/event.rs` | `handle_gui_group_albums`, `handle_gui_emby_root_items`, `handle_gui_shows_new` | Leave with their events (D5) |
| `dispatch/library/event.rs` | `record_artist_item_id`, `handle_gui_artist_name_searched` | Core (b), as `ArtistNameLookup` |
| `dispatch/library/event.rs` | `gui_queue_in_view_row_fetches`, `handle_feed_home_video_selection_failed` | Library effect (c) |
| `dispatch/feeds/feed.rs` | `gui_emby_feed_wants_target`, and the `gui_emby_libs` claim in `handle_feed_home_video_selection_loaded` | Library query and effect (c) |

Also on that list today, and handled elsewhere: `client_arranges_panels` and `visual_slot_controls` (D6a), the `gui_*` field reads in `state/named_queues.rs`, `state/queue_link.rs`, `state/playback.rs` (D6a), `save_prefs` (D4), and `gui_queue_end_offsets` in `queue_summary_projection.rs` (it reads no GUI state; `2-split-tui-gui-clients` renames it).

**(a) GUI half.** The arm in `App::dispatch` or `App::handle_playback_request` becomes a documented core no-op, as `PanelIntent::MoveQueueRow(_) => {}` is today. Every variant stays listed; no wildcard arm. The GUI entry (`input/gui.rs`: its `Command::App` arm and its `Msg::Playback` arm) calls the core first, then its own half. The TUI never sends these commands (the router gates them on `TransportChipKeys`, D6a), so its behaviour does not change.

**(b) Core.** The method is Owner, queue, playback-target, or shared-lookup behaviour (rule D2.3), or both Clients call it. It moves from the GUI module file to a core module beside the code it serves (`state/named_queues.rs`, `state/queue_link.rs`, `dispatch/library/`), and keeps its behaviour. A method that reads a GUI field gets that fact as a parameter or reads a core field instead. `request_new_gui_queue` is renamed `request_new_queue`. The App half of the artist-name lookup is named `ArtistNameLookup`, and its answer stays gated on client presence.

**(c) Library effects and the feed query.** The library event handlers (`handle_lib_event`, `drain_lib_event`, `handle_browse_event`, `handle_music_event`, `handle_feed_home_video_selection_loaded`, `feed_home_video_selection_wanted`, and the core half of `handle_feed_home_video_selection_failed`) take `stores: Option<&mut dyn ClientLibraryStores>`. The core trait is:

```rust
pub trait ClientLibraryStores {
    /// Whether this Client's own selection still wants the landed feed target.
    fn wants_feed_target(&self, app: &App, lib_idx: usize, target: &FeedHomeVideoSelection) -> bool;
    /// Apply one effect that a library event raised.
    fn apply(&mut self, app: &mut App, effect: ClientLibraryEffect);
}

pub enum ClientLibraryEffect {
    RowArtSourcesChanged,
    FeedTargetLanded { lib_idx: usize, selection: FeedHomeVideoSelection },
    FeedTargetFailed { lib_idx: usize, selection: FeedHomeVideoSelection },
}
```

The core collects the effects of one event in a local list and applies them through `stores` after that event and before the next one, the same order as today's direct calls. The trait takes `app` as a parameter, so the same signature serves while the GUI fields are still on `App` (a transitional unit implementor in the GUI modules reads them through `app`) and after they move to `GuiClientState` (which then implements it). `handle_feed_home_video_selection_failed` splits: the fetch-mark retirement and the log stay in the core, and the claim insert becomes `FeedTargetFailed`. The claim retirement in `handle_feed_home_video_selection_loaded` becomes `FeedTargetLanded`. The TUI passes `None`, so it does nothing, as today.

**Order.** The command and library seams land first, while the GUI fields are still on `App`. No step adds a GUI parameter to a core function. Then the GUI fields group into `GuiClientState`, the GUI impl blocks become `impl GuiCore` (callers first), and the state leaves `App` last. Each definition move, per-module reference migration, and separate test-consumer migration is a distinct bounded edit task. A task or group does not need to compile or pass gates to be complete. Dependent breakage between tasks is permitted and recorded against the remaining task IDs; it is not a reason to combine tasks or expand a worker's scope.

Alternatives considered:

- Thread `Option<&mut GuiClientState>` through core functions first and remove it later. Rejected: the first attempt did this (161 files in one step), and the seam removes it again.
- A hook that runs GUI code inside the core handler. Rejected: `gui_queue_in_view_row_fetches` reaches most destinations.
- An async channel. Rejected: it changes the event order, and it cannot answer the feed-wanted query.

### D5. Client-only events leave the shared event enum

The shared event enum loses `GuiGroupAlbums`, `GuiEmbyRootItems`, and `GuiShowsNew`. GUI fetch workers report on a channel that `GuiCore` owns. `ClientTickHost` gets one more hook, `drain_client_events(&mut self) -> bool`, which runs in the shared tick after `drain_lib_events`. The GUI implementation drains its channel. The TUI implementation is a documented no-op that returns `false`. The waker is the same `ClientWaker`, so a GUI worker result still wakes the tick.

### D6. Neutral shared signatures

- Context menus: the core builds the menu entries and the target. Each Client places the menu. No core function takes a pointer cell, and no core state stores one:
  - `build_queue_selection_menu_at` folds into `build_queue_selection_menu`, and the `anchor: Option<(u16, u16)>` parameters in `dispatch/context_menu/menu.rs` and `menu/emby.rs` go.
  - `open_context_menu_for_at`, `open_context_menu_at`, `open_context_menu_at_for_item`, `open_visual_slot_context_menu`, and `handle_mouse_right_click_queue` lose their `x`/`y` (`col`/`row`) parameters.
  - `ContextMenuAnchor::Pointer { x, y }` (`mbv-ui-model`) becomes the unit variant `ContextMenuAnchor::Pointer`: "at the pointer that the Client recorded".
  - The TUI shell, which today passes the cell, keeps it in a shell `Model` field (`menu_pointer: Option<(u16, u16)>`) when it calls the core, and its menu placement reads it from there. Placement does not change. All callers that pass a cell today are in the shell (`shell/messages.rs`, `shell/home.rs`).
- Any other terminal-cell or toolkit type in a core signature or core state gets the same treatment: a neutral value in the core, the conversion in the Client. The neutral `mbv_ui_model::geom::Rect` bounds that `extract-client-core` introduced stay.

### D6a. Client capabilities replace the Client-kind flag

The core names no Client. Where the Clients differ, the core reads a neutral fact. `App::gui_client`, `App::enable_gui_client`, `client_arranges_panels`, `visual_slot_controls`, `App::client_kind`, and `ClientKind` are deleted. The core gets a small flag set (not a struct of `bool` fields: clippy's pedantic `struct_excessive_bools` allows three, and the workspace forbids a suppression):

```rust
/// One thing a Client presents. The core never names a Client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientCapability {
    /// The Client may present one panel only, and hides an empty queue
    /// column in its two-panel layout (TUI: yes, GUI: no).
    TerminalPanelRules,
    /// The Client paints the visualizer and the visual-slot hide control
    /// (TUI: yes, GUI: no).
    VisualSlotControls,
    /// The Client shows one queue entry at a time that the user picks: a
    /// Named queue or the peer's queue (TUI: no, GUI: yes).
    ShownQueueEntries,
    /// The Client paints the transport chips, the seek-10/30 controls, the
    /// playback-target chooser, and moves queue rows by key (TUI: no,
    /// GUI: yes).
    TransportChipKeys,
}

/// The capabilities of this Client. Each Client builds its own value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientCapabilities(u8);

impl ClientCapabilities {
    #[must_use]
    pub const fn with(self, capability: ClientCapability) -> Self;
    #[must_use]
    pub const fn has(self, capability: ClientCapability) -> bool;
}
```

`attach_owner` takes the Client's `ClientCapabilities` in place of `ClientKind`. It builds the first Hello from them (before `App` exists) and stores them on the new `App` before it returns, so the core reads them before the first step. A reconnect builds its Hello from the stored value.

Each existing reader maps to one fact. No behaviour changes:

| Behaviour today | After the change |
|---|---|
| Terminal-width Mini view (`is_mini_view`) | The TUI pushes "one panel only" with `App::set_single_panel(bool)`, computed from its terminal width and `MINI_VIEW_THRESHOLD`, at every point where it records the terminal size today: on resize and on every frame composition (`shell/draw.rs`, the per-frame size). So the first frame of a TUI started in a 70-column terminal is in Mini view, as today. `is_mini_view` reads that fact and `TerminalPanelRules`. The GUI never pushes it, so it stays false. |
| Hidden empty queue column (`queue_column_hidden_empty`), `effective_panel_focus`, `effective_panel_mode` | Stay in the core and read `TerminalPanelRules` and the pushed single-panel fact. GUI code that reads them today calls the same methods. |
| Visualizer and visual-slot hide setters, `visual_slot_hide_in_effect` | Stay in the core and read `VisualSlotControls`. |
| The shown queue (`gui_shown_entry`): `show_peer_entry`, `show_local_current_entry`, `show_entry`, `show_named_queue`, `shown_queue_edit`, `adopt_named_queue_snapshot`, the closed-queue fallback in `adopt_named_queue_list`, and `queue_link_resolution` | Stay in the core. The field is renamed `shown_entry` and the gates read `ShownQueueEntries`. The Owner-projection path keeps the fallback, so the `gui-queue-chooser` rule and ADR 0041 hold. The GUI queue chooser reads the entry through a read method. |
| Router gates `KeyPolicyGate::GuiClient` and `GuiQueueTarget` | Renamed `TransportChipKeys` and `TransportChipQueueTarget`. The router snapshot carries `ClientCapabilities` in place of `gui_client`, and the gates read `TransportChipKeys`. The actions stay in the shared registry, so a rebind works the same in both Clients. |
| `LaunchRestore::Done` set by `enable_gui_client` | The GUI calls a `pub fn skip_launch_restore(&mut self)` before its first step. |
| Hello claim `named-queues` (`ClientKind::Gui` on a local link) | `client_hello_claims` reads `ShownQueueEntries`. |
| Pin-swap non-participation (`attach_owner` calls `claim_pin_swap_participant(false)` for `ClientKind::Gui`) | No capability stands for it. The GUI calls `pin::claim_pin_swap_participant(false)` itself before it calls `attach_owner`, so its first connection already advertises it, as today. |

Alternative considered: keep a `ClientKind { Tui, Gui }` enum in the core. Rejected: every `if kind == Gui` branch is GUI code in the shared core, which the `client-interaction-core` delta forbids.

### D6b. Breakpoints and the tab strip

The core reads only the shared breakpoint class (`mbv-surfaces`) and the pushed single-panel fact. The TUI derives both from its terminal size.

The tab-strip scroll is TUI presentation: `tab_scroll`, `ensure_tab_visible`, `visible_tab_range`, and the tab-strip width rule in `input.rs` (a ratatui `Rect` and `chrome_geometry` over `terminal_width`) move to the shell. Today one core site calls `ensure_tab_visible` (`dispatch/library/cw_library_tab.rs`). It becomes a neutral request: the core sets `tab_reveal_requested` on `App`, and the TUI takes the request after every shell update step and every tick drain, before it paints or resolves a pointer press, and runs its reveal rule with the live panel mode, focus, queue column width, and playback state. So every reader of `tab_scroll` sees the revealed value at the same point as today. The GUI ignores the request; it has its own tab strip.

### D7. Shared leaf crates lose their TUI parts

- `mbv-theme`: delete the `ratatui` feature and the `to_ratatui` method.
- `mbv-images`: the terminal image protocol code (ratatui-image, crossterm) moves out. Decode, cache, and processing stay.
- `mbv-components` and `mbv-render` need both removed pieces. So both go in a new TUI-only leaf crate `mbv-tui-support` (ratatui, ratatui-image): a `ToRatatui` extension trait for `mbv_theme` colour values, and the terminal image protocol. The root package, `mbv-components`, and `mbv-render` depend on it. Nothing in `mbv-gui` does.

### D13. Items in `mbv-render` and `mbv-components` that shared or GUI code uses

`mbv-render` and `mbv-components` stay TUI-only. Each item they hold that the core or the GUI uses goes by one of two rules.

**Rule A: toolkit-free shared logic and data move down to `mbv-ui-model`.** Both Clients import them from there, and `mbv-render` and `mbv-components` import them too.

| Item | From |
|---|---|
| `current_time_secs` | `mbv-render` `screens/feeds_model.rs` |
| `group_album_plan`, `sorted_group_album_order`, `initial_group_artist_sort_key`, `effective_sort_str` | `mbv-render` (`screens/album_plan.rs` and the files `lib.rs` re-exports them from) |
| `book_rows` | `mbv-render` `components/audiobookshelf_book.rs` |
| `HeroImageState` | `mbv-render` `components/tv_wide.rs` |
| `MUSIC_ALBUM_IMAGE_TYPES` | `mbv-render` `components/widgets.rs` |
| `IndicatorStyle`, `StatusIndicator` (data only) | `mbv-render` `components/indicators.rs` |
| `TransportAvailability` | `mbv-render` `components/chrome_player.rs` |
| `hero_content_queue`, `hero_content_feed`, `hero_content_emby`, `hero_content_abs_episode`, `hero_content_abs_book` | `mbv-components` `library_panel/hero.rs` |
| `HeroArtwork`, `ArtworkSource`, `ArtworkShape`, `HeroFacts`, `PanelHeroImagePaint` (data only) | `mbv-components` `library_panel/content.rs` |
| `hero_content_series_with_episode`, `HeroContentData` | `mbv-components` `library_panel/hero.rs` |
| `emby_artwork_policy`, `abs_book_artwork_policy`, `abs_episode_artwork_policy`, `feed_artwork_policy` | `mbv-components` `library_panel/hero.rs` |
| `painted_shape_for_pixels`, `HeroArtwork::painted_shape` | `mbv-components` `library_panel/content.rs` |
| `premiere_number_label` | `mbv-components` `tv_content/episode_rows.rs` |
| `sanitize_url` | `mbv-components` `library_panel/overview_box.rs` |

Where an item's type holds a ratatui value, the item splits: the neutral data moves to `mbv-ui-model`, and the ratatui part stays in its TUI crate as a conversion. Painting code never moves down.

**Rule B: TUI terminal geometry stays in the TUI crates, and its readers move to the shell.** If a core rule needs the result, the TUI pushes it to the core as a neutral value (as with the single-panel fact in D6a). A pushed value is allowed only where today's core already reads a value that the TUI records at the same point (a resize, or a frame composition), and the TUI pushes it at every such point. Where today's core computes the value live inside a dispatch (from the panel mode, focus, queue column width, or playback state of that moment), a push would let the dispatch read a value one step old (the first attempt did this: `edac3bdfc`). Such a rule moves to the shell and the core sets a request (as with the tab reveal in D6b), or the core keeps the rule over neutral inputs.

`AppLayout` (`left_area`, `card`, `root_frame`; the `App::layout` field). It holds ratatui `Rect`s, and three core files read it: `dispatch/actions.rs` (`lib_page_size` from `left_area.height`), `dispatch/context_menu/menu.rs` (the visualizer menu bounds from `left_area`), and `state/projection/tv_wide.rs` (`card.height`, whose projection moves to the shell in the TUI projection tasks). `layout` moves to the shell `Model`. The core keeps one neutral field `left_panel_area: mbv_ui_model::geom::Rect` (default zero), and the TUI sets it with `App::set_left_panel_area` in `compose_root_frame`, with the same value it writes to `layout.left_area` today (zero when the frame is under 4 rows), so readers see the last composed frame, as today. `lib_page_size` and the menu bounds read that field. The GUI never sets it, so it gets what it gets today from the default `AppLayout`: a page size of 1 and no menu bounds.

`MINI_VIEW_THRESHOLD`, `TABBAR_LEFT_RESERVE`, `LEFT_WIDTH_DEFAULT`, `rect_to_presentation`, `LIBRARY_PILL_THRESHOLD`, `chrome_geometry`, `ChromeGeometryInput`, `tab_strip_text_width`, `wide_library_panes`, `normalize_queue_column_width`, `wide_hero_fits`, `visible_tab_range`, `service_state_color`, `components::card`, `hero_artwork_box`, `MusicWideRenderCtx`, `LibraryListRenderCtx`.

These lists are a start, not the definition. The shared-item and geometry inventories and the final boundary audits in `tasks.md` define the remaining set. An item that source inspection or later integration checks find goes by the same two rules. Record it as its own bounded follow-up task before dispatch; finding it does not authorize a worker to expand the current assignment.

## Risks / Trade-offs

- [The change touches most of `src/app/`, so one refactor assignment can exceed an agent's context] → Dispatch one bounded edit row or less to a fresh worker. Separate definition moves, caller-module migrations, test-consumer migrations, and cleanup. Never combine rows to reach a compile-green state. Complete an edit task by inspecting its named edits, even if dependent code is temporarily broken. Compilation and passing gates are not task or group completion requirements; integration checks assess the assembled change separately.
- [A boundary search finds unfinished dependent migrations] → Keep the finding as an explicit handoff to a named remaining task. Searches inventory work and final audits assess the boundary; they do not require every edit row to eliminate all matches. Check each row's named symbols and file scope before dispatch.
- [The plan describes code from another branch, not `main`] → D4a's *Today* is measured at HEAD. A row's premise names the symbol and file it depends on, and the row is checked against `main` before dispatch.
- [A pushed value is one step old] → D13 Rule B allows a push only at the points where the TUI records the value today; a live rule becomes a request (D6b).

## Migration Plan

No user migration. The prefs file format and key names do not change. No packaging changes.
