# Design: add-gui-library-panel

## Context

See `proposal.md` for the why and `specs/` for the requirements. The canvas reference is a verbatim local copy of the canvas files in `canvas/`: `canvas.json` (the index, with the SPEC note on multi-select pills), `Library.dc.html` (*Library — Music › Hip-Hop*), `Main.dc.html` (*Queue panel — expanded*), `Collapsed.dc.html`, and `Menus.dc.html`. The copy is a source to read; its `./support.js` runtime belongs to the canvas host. The live canvas is `https://claude.ai/artifact/D8uLGS6NyCeHd6ed7PHgsr`.

The canvas is the specification for the GUI Library panel. The GUI does not keep parity with the TUI, and the TUI does not change (ADR 0036). Where the canvas is silent, this design decides, and it says so.

What the code holds today:

- The GUI core thread (`crates/mbv-gui/src/core_thread.rs`) owns an `App`, emits `GuiWindowFrame` on `mbv://frame`, and takes `key`, `pointer`, and `report_layout` commands (`docs/architecture/gui-client.md`). The front end forwards every keydown and pointer event and interprets none (`ui/src/lib/bridge/input.ts`).
- GUI-only presentation state already lives on the `App` in `src/app/gui_client_state.rs`, and the TUI never reads it. That module is the precedent for the GUI Library's core half.
- `CoreThread::new` runs only `App::start_emby_client_fill`, an authentication-only Emby worker, and sets Queue Panel focus on its own `FocusStack`. So `App::libs` stays empty in the GUI. The TUI's Service bootstrap is `Model` code: `Model::spawn_startup_services` (the Emby and Audiobookshelf startup workers, `start_feed_fetch`, capability registration), then `Model::fetch_home_at_startup` and `Model::init_idle_feed`.
- The `App` holds the Service data of every destination: `libs` (each library's `nav_stack`), the Audiobookshelf browse states, and `feed_tab`. Home's content is the exception: the TUI shell holds it in `Model::home_content`.
- For a Grouped Music library, `nav_stack` holds the group level and the album level of one group. `App::switch_music_group` and `App::select_music_group` replace that one album level, and `App::spawn_browse` lands each fetched page in the `nav_stack` level whose parent matches. The `App` also holds `album_artist_cache` (warmed for every group by `App::spawn_music_group_warmup` when the Emby Service completes), `album_tracks_cache` (filled by `App::fetch_album_tracks`), and the card image cache (filled by `App::fetch_card_image`).
- The `App` loads the TUI launch state at construction for both Clients (`LaunchRestore::Pending`), and the restore settles `App::tab` when the catalog lands. The GUI never saves a launch state.
- `App::effective_panel_focus` and `App::effective_panel_mode` apply two TUI rules: the terminal-width Mini check (`is_mini_view`) and the hidden empty queue column (`queue_column_hidden_empty`). The GUI's `App` keeps the default terminal width, so an empty queue forces Library focus in the GUI too.

## Goals / Non-Goals

**Goals:**

- The GUI Library panel looks and behaves as the canvas specifies.
- The Library loads its content by the same Service steps as the TUI.
- Music is complete and accepted before the other destinations start.

**Non-Goals:**

- No change to the TUI's paint, keys, or behaviour, and no TUI Library code moves.
- No Owner-side, ctrl, or Player change. No new dependency.
- No restyle of the Queue panel. The Queue panel keeps its core-held interaction state.

## Decisions

### D1. The front end owns the Library's interaction; the core owns data and actions

| Front end (Svelte) | Client core (`App` and `CoreThread`) |
| --- | --- |
| Cursor, hover, scroll, open tree nodes | The active destination (`App::tab`) |
| Focus inside the panel (pill row, list, Hero, tracklist) | The selected collections of each destination |
| Filter text, Search query text | The item whose detail the Hero shows |
| The open Hero overlay and item menu | Service data, its loading, and the artwork |
| The Library's keys while it has Panel focus | Panel focus (the `FocusStack`), the actions |

The data selection is core state because the core needs it to decide what to load and what to save at exit. The front end changes it only by a command, and the frame carries it back, so the pill row paints the core's set.

The GUI Library panel uses no TUI component, no `mbv-surfaces` Surface state, no `GuiSurfaceLeaf`, and no `[keys]` action. Its pointer input never reaches the core's gesture recogniser or hit map.

*Alternative:* port the seven TUI destination owners to shared Surface state. Rejected (ADR 0036): it refactors the TUI and binds the canvas behaviour to the TUI's rules. *Alternative:* GUI-only interaction state in the core. Rejected (ADR 0036): every hover and scroll crosses the bridge, and the Library keys still meet the TUI's bindings in the shared router.

### D2. The bridge: one frame section and one command

`GuiFrame` gains `library: Option<GuiLibraryFrame>` (`crates/mbv-gui/src/bridge/frame/library.rs`). It carries the destination tabs, the data selection, and the active destination's content: the pills with their kind (collection or single), the list rows, the Hero's detail, the item menu's entries, the loading and empty states, and the artwork as `data:` URLs from the image cache. A Library list can hold thousands of rows, so the section carries a revision, and a frame re-carries the content only when the revision changes, as `src/artwork.rs` does for the visual slot. A window-content reload's `report_layout` re-arms it. The front end keeps the last carried value.

A new Tauri command, `library(GuiLibraryCommand)`, pushes a `CoreInput::Library` into the core channel, as the other commands do. The variants: open a destination, set a destination's selected collections, show an item's detail, open an album node (load its tracks), take Panel focus, play, play next, add to queue, shuffle an artist, run an item-menu entry, and (Phase 2) run a search query and activate a result. The core thread applies each through one public GUI entry on the `App` (`src/app/gui_library.rs`, beside `gui_client_state.rs`), which calls the existing `App` actions. Item identities are stable Service ids, never row positions.

The projections that build the section are public `App` methods in `src/app/gui_library/`. They read `App` data only, so the TUI's projections and paint do not change.

### D3. Keys: the Library handles its keys first

The front end routes each keydown in this order:

1. While a core overlay is open (the row menu, the track popover, the playback-target chooser, the confirmation modal) or prefix mode is armed (both are in the frame), the key goes to the core as today.
2. While a text field in the Library has focus (the `/` filter, the Search field), the field takes the key. Esc closes the field, and Enter accepts it.
3. While the Library panel shows: Tab and Shift+Tab move focus to the next or previous pane (pill row, list, Hero, Queue panel), the canvas's *tab pane*. `1` to `7` open the first to seventh destination tab after Home, the canvas's *1–7 library*. With global search presented, `Ctrl+K` focuses the Search field.
4. While the Library panel has Panel focus, the Library keys below run in the front end.
5. Every other key goes to the core's keyboard router, as today. So Space plays and pauses when no pill has focus, and `[` and `]` seek.

The Library keys (the canvas names the first six; this design adds the movement keys, because the canvas is keyboard-first, note `n1`):

| Key | Effect |
| --- | --- |
| `/` | Open the filter of the list. |
| Enter | Play the item under the cursor, or press the focused control. |
| `n` | Play next the item under the cursor. |
| `a` | Add the item under the cursor to the queue. The add applies the library playback policy's autoload rule to the Library's routes (D8). |
| Space | On a focused pill: toggle it. |
| Alt+Space | On a focused pill: solo it. |
| Up, Down, Page Up, Page Down, Home, End | Move the cursor in the focused list or tracklist. |
| Right, Left | In the tree: open or close the node; on the pill row: move the focused pill. |
| Esc | Close the open filter, overlay, or menu. |

The handler runs before `input.ts`'s forwarder and stops a handled key from reaching it. The front end sends the core no key that it handled. The Queue panel's keys, the GUI-only chords `z`, `q`, and `c`, and every `[keys]` binding keep their effect whenever the Library does not take the key. The GUI-only chords `/` (Queue filter) and `a` (audio popover) work with Queue Panel focus. With Library Panel focus, the Library takes `/` and `a`.

### D4. Panel focus and the arrangement

Panel focus stays in the core's `FocusStack`, and the frame's `GuiFocus` carries it. `CoreThread::new` mounts `ComponentId::Library`. A press inside the Library panel sends the take-focus command, and a press in the Queue panel moves focus as today. Tab across the panel boundary sends the same command. The core sets the stack and `App::panel_focus` together.

The GUI's `App` does not apply the TUI's terminal-width Mini check or its hidden empty queue column: the GUI always paints the Queue panel, and it decides its own arrangement. A GUI setting on the `App` makes `is_mini_view` false and `queue_column_hidden_empty` false for the GUI, so the core's key gates see the stored Panel focus. The TUI's values do not change.

The front end picks the arrangement from the measured Library column width (the window width minus the Queue panel's current width, 400 px expanded or 72 px rail), with the canvas sizes as thresholds: below 480 px (the 320 px list plus the pill row) is Mini, from 880 px (the 320 px list plus a 560 px Hero) is Wide, and between is Narrow. Wide also needs 600 px of height. Mini shows only the panel with Panel focus. Narrow shows the list and opens the Hero as an overlay when an item is picked. Wide shows the Hero beside the list. The thresholds are front-end constants beside the queue panel's state constants. The core does not classify the size, so `mbv_surfaces::breakpoint` stays unused by the GUI.

### D5. Music data: the collection store and the projection

A Grouped Music library's collections are its groups (the group level in `nav_stack`). Its pills are the groups in their level order, and every group pill is a collection pill.

The `App` gains a GUI collection store: per Grouped Music library, the full album list of each selected group, keyed by group id. A group's first selection starts a fetch with the client call that `App::spawn_browse` makes (`get_items_sorted` with the group as parent, sort `SortName`), paged until the total is reached. The completion lands in the store and not in `nav_stack`, so the TUI's navigation state and fetches do not change. The store keeps a loaded group for the session, so a deselected group selects again without a fetch. A group that fails to load keeps the others and shows a failure line for that group.

The Music projection builds the list from the selected groups' albums. `mbv_render::group_album_plan` (the function that `App::wide_music_render_ctx` calls, here with no settled catalog) gives each album its `(artist, year, name)` facts and its stable `ArtistKey` from `album_artist_cache`. The projection then builds the union: one artist row per `ArtistKey`, in the artist sort-name order of `mbv_render::sorted_group_album_order` (articles stripped), each with its albums, and each album with its tracks when `album_tracks_cache` holds them. Each artist row carries its sort letter, so the front end paints letter headings and the rail without a second sort rule. An album node that opens sends the open-album command, and the core runs `App::fetch_album_tracks` when the cache does not hold the tracks.

The Hero shows the picked album: the artist header (the artist, the selected collections, the album count, and the track count and total time once the artist's album tracklists have loaded), the album facts, the square artwork, and the tracklist. The core requests the album art with `App::fetch_card_image` and encodes it from the image cache as `src/artwork.rs` does. A picked track picks its album.

### D6. Multi-select collection pills

The canvas SPEC note sets the rules. A press on a collection pill toggles it, and an Alt-press solos it. Space and Alt+Space do the same on the focused pill. Turning off the last selected pill leaves the empty set — the show-all selection: the union of every group, the same content every pill selected produces, loaded through the same per-group fetch and cache. With two or more selected, the row shows *N selected · clear*, and clear keeps the first selected pill in pill order. The list header reads the selected names joined by ` + `, or *Name · sort name* for one. A single pill (Latest, a filter, a letter bucket) clears the set and selects alone.

The front end computes the new set from the press and sends it with the set-collections command. The core stores it per destination and loads the new groups (D5). Each destination keeps its set while the GUI runs, and the launch location saves it (D9).

Collection pills by destination (Phase 2 uses the same rule; a pill is a collection when it narrows the list to one content set that the Service holds):

| Destination | Collection pills | Single pills |
| --- | --- | --- |
| Music (grouped) | every group pill | none |
| Podcasts | every show pill | Latest, the played-state filters |
| Feeds | every subscription pill | Latest, the watched filters |
| Emby library in feed-group mode | every feed-group pill | Latest, All |
| Shows, Movies and other Emby libraries, Home, Audiobooks | none | every pill |

### D7. The list and the letter rail

The list is the canvas tree: artist rows (30 px), album rows with the year (30 px), and track rows with the track number (28 px), with 24 px letter headings above the first artist of each letter. A press on an artist or album row opens or closes it and picks it; a press on a track moves the cursor onto the row and picks its album — the pick's Hero is album-scoped, so the cursor is the press's visible half, as the Queue panel's click moves the queue cursor. A double-click on a track row of the list or of the Hero's tracklist adds that track to the queue through the add path's autoload rule (D8). The cursor and the picked item are different: the cursor moves by keys, and a press picks the item for the Hero. Enter plays the item under the cursor and never picks it: the GUI is mouse-driven for picking, and the TUI's Enter-picks behaviour is its own compromise. The list paints only the rows in view (fixed heights, native scroll), so a long artist list stays fast.

The letter rail shows A to Z beside a list sorted by name. A letter with no rows is disabled. A press scrolls to the first row of that letter and moves the cursor there. The canvas gives the rail no key. Its keyboard path is the `/` filter. The filter narrows the rows to the names that contain the text, without case.

### D8. Hero actions and play next

Play, Add to queue, and Shuffle artist call existing `App` actions: `App::play_or_activate_lib_item`, `App::play_album_track`, and `App::play_music_albums` (with shuffle for Shuffle artist, over the artist's albums in the current union), and `App::enqueue_lib_item`.

The Library's own play and add apply the library playback policy's autoload rule (the Grouped Music tree's track activation): with autoload on, a track whose album the track cache holds replaces the queue with that album's cached playable tracks in disc/track order starting at the selected track, and the add queues the album's cached tracks from the selected track onward; with autoload off, the play and the add act on the track alone. Every other item takes the shared paths unchanged, and the TUI's own add paths keep their per-item appends.

The Library's explicit play and the explicit Shuffle artist skip the TUI's populated-queue replace-confirm: the shared folder and album-expansion paths run with the confirm off, so a play or a shuffle is never gated behind that prompt and a populated queue does not swallow the Hero's buttons. The local saved-playlist protection stays, and its prompt presents in the GUI's confirmation overlay.

The frame carries the playhead's Emby track id from the Queue panel's own authority — the Owner status and the playback queue's item at the active index — so a tracklist row and a list track row whose track plays paint the canvas's playing look: the green fill with dark ink, as the Queue's active row paints, over the cursor's fill.

Play next is new: `App::play_lib_item_next(lib_idx, item)`. It resolves the item with the same route and admission checks as `App::enqueue_lib_item` (folder expansion, playability, route conflict), then sends the Owner's answered Append with `before` set to the slot after the active slot (`QueueOp::Append { items, before }`). With no active slot, or when the active slot is the last one, it sends `before: None`. It never moves a slot that is already in the queue. The TUI has no producer for it.

The Hero's controls show their keys (`n`, `a`). The tracklist's hover controls run play next and add to queue on that track. More actions opens the item menu: the core builds its entries with `App::build_context_menu_for` (the private builder behind `App::open_context_menu_for`, widened to `pub(in crate::app)`), the frame carries them, the front end paints them in the row-menu look of `canvas/Menus.dc.html`, and a chosen entry runs through the id-addressed menu-entry executor (`App::execute_context_action` after the GUI's id-resolved Play and Add-to-queue rewiring, which are never resolved through the TUI cursor). The GUI never arms the TUI's `pending_overlay`. `execute_context_action` finds the library through `App::context_menu_lib_idx`, which needs Library Panel focus and an Emby library in `App::tab`; D4 keeps both true while the Library has Panel focus.

Each carried entry names its index in the shared builder's order, so a front end that paints only some entries (the Hero's More actions) still executes the entry it painted. The Hero's More actions hides the entries its own buttons already send (`Play`, `Play All`, `Add to Queue`) at paint time — the menu names only what no button offers — and paints no More actions control at all when the filter leaves it nothing.

A right-click on a Library list row (artist, album, track) or a Hero tracklist row opens the same item menu as a full list: the `rowMenu` command names the row's stable Service id, the frame's `rowMenu` section carries the TUI menu builder's full entries for that item, and a chosen entry runs through the same menu-entry path. A right-click carries no menu for an item the Service data no longer holds. The music kind's mark entries stay absent, per the context-menu spec's music rule.

### D9. The GUI keeps its own launch location

The GUI's `App` starts with `LaunchRestore::Done`, so the TUI launch state never moves the GUI's tab or Panel focus. The GUI keeps its own file (`mbv_config::load_gui_launch_state` and `save_gui_launch_state`, additive, beside the TUI launch state): the destination, the selected collections of each destination, and Panel focus. The core saves it at orderly exit, before `client_exit_teardown`, and never while the window is open. At start, after the catalog lands, the core opens the saved destination, applies its saved sets (a saved group that no longer exists is dropped; when none remain — a saved set the library emptied, and a destination with no saved entry, a fresh launch's case — the empty set applies, the show-all selection), and sets Panel focus. The restore applies to a destination that holds no selection and never clobbers one the session already made. While the GUI presents only some destinations, a saved destination that it does not present falls back to the first presented one. A start with no saved location — or one that fails to load — restores through the same path: the first presented destination, Library Panel focus, and no saved collections, whose empty set is the show-all selection.

The TUI launch state does not change: the TUI never reads the GUI file, and the GUI never writes the TUI file.

### D10. The Library Status bar

Under the Library column, the Library Status bar shows the canvas's key hints on the left (`/ filter`, `⏎ play`, `n next`, `a queue`, `tab pane`, `1–7 library`) and the location on the right: the destination, the selected collection names, and the picked item's artist or title, joined by ` › `. An empty selection is the whole library, so the location names it `All`, as the list header does. The hints are the GUI's fixed keys (D3), so they are fixed text. While prefix mode is armed or a toast shows (the shared `GuiStatusBar`), that replaces the hints, and the location stays. While the Library panel does not show, the Queue footer paints the prefix and the toast as today.

### D11. The service bootstrap becomes a shared Client step

The TUI-only bootstrap steps move into one `App` step, `App::start_client_services`, that both Clients run once after the attach: the Emby startup worker, the Audiobookshelf startup worker, `start_feed_fetch`, and capability registration. `Model::spawn_startup_services` calls it in place of its body, at the same point, so the TUI's order and timing do not change. The GUI core thread calls it in place of `start_emby_client_fill`, which is deleted with its worker and tests.

The GUI keeps its playback-target discovery. The client fill starts `spawn_sessions_load` and `spawn_cast_discovery` for a cached client and on its completion; the full Emby startup starts neither. So the GUI core thread starts both after `start_client_services` when the attach brought a cached Emby client, and in its `drain_startup_workers` host when the drain reports a successful Emby startup (`StartupWorkerDrain` gains `emby_started`; the TUI host ignores it).

On a launch with a cached Emby client no Emby startup worker runs, and only `App::fetch_home` builds the library tabs. So the `App` half of `Model::fetch_home_at_startup` moves into `App::load_home_at_startup() -> Option<HomeContent>`: `fetch_home`, then on success `spawn_home_recently_added(Emby)` and the status clear, and on failure the "Couldn't load home" toast. Both Clients call it right after `start_client_services`; the `Model` keeps the Home half (`home_content.loading` and the assignment). The GUI drops the returned content until Home lands (unit 10). `init_idle_feed` joins the shared step in unit 10.

### D12. Phase 2: the other destinations and Search

Each destination paints the Music slots with its own data: a pill row, a list, a Hero (with landscape, portrait, or square art, or no Hero), and an optional Workspace list in the Hero (Shows' seasons and episodes, Audiobooks' chapters). Each unit adds its projection in `src/app/gui_library/` from the `App` data that the Context section names, and its commands. Home's content moves from `Model::home_content` into the `App` (the TUI reads it there; its paint does not change), and the GUI stores the content of `App::load_home_at_startup` and of the startup drain's `home`. Each unit's first step reads its destination's data path and records it in this design before the code.

Search: the top bar's Search field (*Search everything*) sends its query with the search command. The core runs `App::spawn_search_sidebar_query`, and the GUI tick host runs `drain_search_results` in place of its no-op. The results paint below the field in the list-row look. Enter or a press activates a result through `App::activate_search_result`, and Esc closes the results.

## Risks / Trade-offs

- [A window-content reload resets the Library's cursor, scroll, and open nodes] → The data selection survives in the core, so the destination, the pills, and the Hero's item stay.
- [The Library keys are GUI keys that `[keys]` does not configure] → Accepted (ADR 0036). Every key the Library does not take still reaches the router and `[keys]`.
- [A union of collections makes one fetch per selected group] → The store keeps each loaded group for the session.
- [The shared bootstrap touches TUI startup code] → The TUI keeps its call order; its startup and lifecycle tests are the proof.
- [The thresholds may feel wrong live] → They are front-end constants; the Music gate's live test checks them.

## Migration Plan

1. **Phase 1 — Music from end to end:** the shared bootstrap (D11), the GUI `App` settings (D4, D9), the Library command and frame section (D2), the Music store and projection (D5), play next and the item menu (D8), the launch location (D9), and the front end: the arrangement, the top bar tabs, the pill row, the list and rail, the Hero and its overlay, the tracklist, the Library Status bar, and the keys (D3). The top bar shows only the presented destinations (no stub).
2. **Gate.** A `/code-review` round over Phase 1 and a user live-test round. Phase 2 starts only after the user accepts Phase 1.
3. **Phase 2 — the other destinations and Search (D12).**

Rollback is a per-unit revert. The only stored-state change is the additive GUI launch-location file.
