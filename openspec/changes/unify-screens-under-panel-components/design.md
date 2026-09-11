## Context

See `proposal.md` — Why.

**Implementation base: `main`** (user decision), starting from the commit that adds this change
(`9e59a29a`, whose only code-tree parent is `18e96392`). The `refactor/unify-wide-hero-content-box-frame`
branch (60 commits, including `arrangements/wide_hero_composition.rs`) is **not** merged and is
discarded; nothing in this change depends on it, and none of its symbols (`compose_wide_hero`,
`ListChromeVariant`, `WideHeroVariant`, `compose_list_chrome`) exist on the base. Task 0.1 re-verifies
every citation below against the actual base before work begins.

Facts that shape the approach, verified on `main` at `18e96392`:

- **The frame is a legacy base frame plus overpaint.** `Model::draw_frame` (`src/app/shell_run.rs:60-100`)
  calls `App::compose_base_frame` (`shell_draw.rs:84`) → `render_main` (`:177`) → `paint_legacy_chrome`
  (`:399`: `render_legacy_backdrops`, `render_tabs`), `render_card`, the queue-only
  `render_player_panel` pair, `render_queue_panel_frame` + `render_queue_status`, `render_library`
  (`widgets.rs:515`, now only publishing `layout.*_area`) and `render_status_bar`. Then a hand-ordered
  list of `render_*_component` calls paints the mounted components over it.
- **`render_main` mutates state while painting**: `library_tab_pending` resolution, the
  `normalize_stale_browse_destination` fallback, and writes to `layout.card`, `layout.queue_*`,
  `layout.selector_tabs`, `layout.breadcrumbs`; `compute_frame_layout` clears `card_image_states`,
  clamps and saves the queue column width, and forces `mini_view_focus` on resize.
- **`LayoutMain` (`layout.rs:67`) is a paint-to-input side channel**: ~30 rect fields written during
  paint and read by input (`tabs_hitmap`, `selector_tabs`, `left_item_rows`, `tv_wide_season_tabs`,
  `queue_area`, `selected_item_rect`, …) in 41 non-test files (75 including tests). `LayoutPlayback` does the same for transport and
  status-bar pill rects.
- **Destination components wrap per-destination painters.** `TvWorkspaceComponent::view`
  (`components/tv_workspace/mod.rs:536`), `BrowserComponent::view` (`browser/mod.rs:470`),
  `MusicWorkspaceComponent::view` (`music_workspace.rs:577`) and `FeedsComponent::view`
  (`feeds.rs:489`) build `LayoutMain::default()` and call `render_wide_tv_with_ctx`,
  `render_wide_movies`/`render_narrow_browse_with_ctx`, `render_wide/narrow_music_group_with_ctx`,
  `render_feeds_content`. Home and the two ABS components call their own painters
  (`render_home_content`, `render_audiobookshelf_{book,podcast}_content`) with private geometry structs.
- **Pane placement is already shared; everything inside it is not.** Every Wide destination goes
  through `wide_hero_split` (list ~40% left, hero right). Inside, each painter re-paints the list panel
  fill + `wide_hero_browser_border` itself, picks `LeftPaneFocus` (10 files), `SelectedRowSurface`
  (15 files), `WideHeroContentBoxSurface` (Music only), and stores pill hits three ways
  (`layout.selector_tabs`, Home `pill_regions`, ABS geometry structs).
- **The mounted set is flat**: `ComponentId::{Home, Browser(BrowserKey), TvWorkspace(BrowserKey),
  Feeds, Playback, Queue, QueueBoundary, WideHeroBoundary, …}`. TV is two mounted components for one
  library (`TvWorkspace` Wide, `Browser(TvShows)` Narrow) with a breakpoint hand-off
  (`hand_off_tv_breakpoint`, `apply_pending_inline_search_transfer`). The router (`router.rs`,
  `key_policy.rs`) references no destination id.
- **Painting starts effects**: `render_card` (`card.rs:257`), `compact_banner_layout_with_overview`
  (`detail.rs:173`), the Home hero (`home_hero_emby.rs:244,261`) and `paint_music_image`
  (`album_art.rs:77`) call `fetch_card_image`, each with its own image-type chain and cache key.
- **Dead branches**: `LibraryListRenderCtx.search_query` is `Some` only inside the Inline Search painter
  (`list_rows.rs:281` via `inline_search.rs:31`); Music/TV `ctx.list.is_search_active()` branches are
  unreachable. The Grid presentation (`GridMediaList`, `NarrowBrowseControl::Grid`) serves only non-hero
  catalogs, and no library in use lacks a hero.

## Goals / Non-Goals

**Goals:**

- One composition owner per screen region, reached from the root, with no painting outside
  components.
- A Library panel whose content types are the only way to put anything on a library screen, so
  divergence is unrepresentable rather than discouraged.
- Every divergence found in the inventory resolved in one direction (the Emby direction), recorded
  once (D14).
- Every slice leaves a shippable app with exactly one painter per surface.

**Non-Goals:**

- Changing Service, Player, queue, persistence or protocol authority; changing keyboard precedence
  (the router and `key_policy.rs` are untouched except where a focus id is renamed).
- Changing list-row painting, row mechanics, or the canonical media-list owner beyond deleting Grid.
- Adding artwork sources that do not exist today (Feeds entries have none; they keep the placeholder).
- Removing the queue title bar (the folded change's deferred question stays deferred).
- Any enforcement mechanism outside the code (tests of sameness, `ast-grep` rules, scripts, CI gates).

## Decisions

**D0 — Unification by construction is the governing decision.** Every later decision exists to make a
cross-screen difference impossible to express, not to name it. The test applied to every type in this
change: *can a destination use it to look different from its siblings?* If yes, the type is wrong. A
caller-selected variant arm with one user is a defect (ruling from the user); a difference is allowed
only when it follows from content every destination can supply (an artwork kind, an empty slot).
Emby screens are the reference; ABS and Feeds conform (D14). Each task section in `tasks.md` opens
with the divergence it removes.

**D1 — The root composes a fixed panel set; the base frame is deleted.**
*Removes:* the base-frame-plus-overpaint composition (every screen). The draw path becomes:
compute one paint-free `RootFrame` (extending today's `chrome_geometry` / `FrameChromeGeometry`) that
places `TabPanel`, `LibraryPanel`, `LibraryPlaybackPanel`, `QueuePanel`, `QueuePlaybackPanel`,
`StatusBarPanel` and `QueueBoundary` for the current Panel mode, then view each placed panel, then the
overlay stack. The panel order is data in `RootFrame`, not a hand-written call list. Column backdrops
become each panel's own fill. `compose_base_frame`, `render_main`, `paint_legacy_chrome`,
`render_legacy_backdrops`, `render_library`, and every `render_*_component` shell method are deleted.
Everything `render_main`/`compute_frame_layout` mutates moves to the sync pass
(`sync_mounted_surfaces`) before the draw: tab-pending resolution, stale-destination normalization,
resize handling (image-state clear, queue-width clamp + save, mini-view focus).

*Incremental path:* `compose_base_frame`/`render_main` is one monolithic body, so S0 introduces
`RootFrame` as **data only** and `draw_frame` keeps delegating to `compose_base_frame`; each later slice
moves one panel's painting out of `render_main` into its component in the same commit (D16), until S11
deletes the empty remainder.

*Mount rule (live ICF: a mounted component never gets an empty rect):* `RootFrame` places a panel only
in the Panel modes where it paints, and the sync pass mounts/unmounts it to match — Tab, Library and
Status bar panels: library column visible; Library playback panel: queue column hidden; Queue panel
and Queue playback panel: queue column visible; `QueueBoundary`: two-panel layout only. `QueueBoundary`
stays the mounted boundary component it is today, but reads its rect from `RootFrame` instead of
`LayoutMain.queue_boundary_area` (task 1.4).

*Alternative rejected:* keep `compose_base_frame` as a thin "root" that calls panels — it keeps the
shell as a painter and the `App` borrow in the draw path.

**D2 — One mounted `LibraryPanel`; destinations become embedded content owners.** (User decision.)
*Removes:* per-destination mounted components that each own a whole screen. `ComponentId::{Home,
Browser(_), TvWorkspace(_), Feeds, WideHeroBoundary}` collapse into `ComponentId::Library`. The
`LibraryPanel` AppComponent is the event boundary for the library area: it owns focus, mouse
subscription, skeleton geometry, the Wide split-boundary drag gesture (today's
`WideHeroBoundaryComponent`), and a map of embedded content owners keyed by a `LibraryKey`
(`Home | Feeds | Service(BrowserKey)`), retained while the library is in the catalog (today's
`reconcile_destination_mounts` rule, moved inside). Content owners follow the embedded-media-list
pattern: plain types, never mounted, focused, subscribed or given a `ComponentId`; they keep
cursor/scroll/focus/drafts/media lists and emit the same typed `Msg`s as today. The shell pushes
content to `LibraryPanel` addressed by `LibraryKey` instead of downcasting per-destination components.
Focus between Library and Queue is unchanged (Panel focus); pane focus inside the library is the active
owner's state.
*Alternatives rejected:* destinations stay mounted and call a panel painter (the panel would be a
helper they choose to call — the failure mode of #691); two mounted Wide/Narrow panels (owners would be
handed across on every breakpoint change).

**D3 — The only paint input is `LibraryPanelContent`.** *Removes:* free painters, private geometry
structs, `LayoutMain` threading. The active owner produces, per frame:

```text
LibraryPanelContent {
    selector: Option<SelectorRow>,        // one pill bar: labels, active index
    controls: Option<ListControls>,       // one row: optional pills + optional label
    list:     ListSlot,                   // Media(&mut dyn PanelList) | Empty { loading: bool, text } | Search(&mut InlineSearch)
    hero:     Option<HeroContent>,        // breakpoint-neutral: Wide header AND Narrow inline hero
}
HeroContent { facts: HeroFacts, overview: Option<String>, workspace: Option<Workspace> }
HeroFacts   { title: String, meta_rows: Vec<String>, artwork: HeroArtwork }   // plain data, no Span/Style/width
HeroArtwork { shape: ArtworkShape, source: Option<ArtworkSource> }             // built only by the policy (D5)
Workspace   { selector: Option<SelectorRow>, list: &mut dyn PanelList, focused: bool }
```

`PanelList` is one object-safe trait over the canonical media-list presentations (view into a rect,
resolve a point from retained geometry, current selected/detail rects, accept a closed paint policy,
and `set_presentation(Wide | Inline, anchor)`), implemented once by the shared media-list carrier for
every `Target`. It exists so per-destination `Target` types never force a per-destination `ListSlot`
arm. The **panel** drives `set_presentation` from its own Wide/Narrow choice (today the destination
sets `MediaListCarrier::active`, `carrier.rs:20-23`), computes the Inline `desired_detail_rows` from
`HeroContent`, and sets the paint policy — focus, `SelectedRowSurface` by slot, and the throbber from
`ListSlot` loading state; destinations pass none of these.

The panel module owns these types and exposes no rect, surface, style, focus-kind, header-arm or
variant field. `ListSlot::Search` makes the panel place the Inline Search box in the Selector row's rect
— reserved for it even when the destination supplies no `SelectorRow` — and its results in the list box
(today the destination passes `pill_area` to `render_inline_search`, `inline_search.rs:31`); the search
component keeps its session and painter.
The panel paints the skeleton, calls the embedded list presentations' `view` into slot rects it
computed, retains slot geometry, and resolves pointer input into slot events
(`SelectorPicked(i)`, `ControlPicked(i)`, `WorkspaceSelectorPicked(i)`, list delegation as today)
handed to the active owner, which translates them into its existing `Msg`s. Placeholder text and
loading/empty state are content (`ListSlot::Empty`), positioned by the panel.

**D4 — Wide and Narrow are the panel's two skeletons, chosen by the existing predicate.**
*Removes:* per-destination breakpoint handling and the TV two-component split. `wide_hero_fits` stays the
single predicate. Wide skeleton: Browser pane (Selector row + spacer, List controls row, list box with
fill + border) | gap | Hero pane (resting or focused surface, Hero header, overview box, Workspace).
Narrow skeleton: Selector row, List controls row, `InlineMediaBrowser` with the one inline hero form
(D7). The existing arrangement primitives (`wide_hero_presentation`, `pill_bar_areas`,
`wide_hero_browser_pane`, `wide_hero_hero_content_box`, `place_media_list_below`) are used by the panel
from S4, and become private to the panel's arrangement module in S11 (task 12.3), once the last
un-migrated destination painter that calls them is deleted — flipping privacy earlier would break the
build while destinations 6–11 still use them.

**D5 — One hero producer per content type, one artwork policy, three header arms.** (User decisions.)
*Removes:* three Emby header painters (`HeroData::render_content` wide path, TV's
`paint_hero_content` + `wide_hero_slots` path, Music's `paint_wide_hero_text` + `wide_music_left_layout`),
ABS/Feeds header painters (`paint_feed_hero`, `render_book_hero`, `render_podcast_hero`), per-producer
styled metadata, and per-destination image chains and cache keys (Home `"{id}:pwr_kw"` +
`keep_watching_hero_image_types`, `home_hero_emby.rs:244-262`; Music `inline_album_art_cache_key` +
`MUSIC_ALBUM_IMAGE_TYPES`, `album_art.rs:77`; the `card.rs`/`detail.rs` chains).

*One producer per content type.* `HeroContent` is built only by `hero_content(&item)` functions in the
panel module, one per provider content type: `EmbyItem`, `QueueItem`, ABS book, ABS podcast show and
episode, feed entry. Every destination calls the producer for its item — Home calls the same ABS-book
producer the Books tab calls — so the same item has one set of facts, one artwork and one header
everywhere. `impl Hero for QueueItem`/`EmbyItem` (`hero_model.rs:44,112`) become inputs to these
producers, not a second path.

*Metadata (user decision).* `meta_rows: Vec<String>`, plain text, one entry per row, no width. The
header painter colours row *n* with `HERO_META_ROLES[n % 3]`, three theme roles defined once in
`render/theme` (initially `TEXT_DETAIL_META`, `TEXT_METADATA`, `TEXT_SECONDARY`, the three colours the
Emby headers already use), and owns truncation and wrapping. ABS progress (`42%`, `Finished`) is just
text in a row.

*Artwork policy (user decision), in one function `artwork_policy(&item) -> HeroArtwork`:* Music
(albums, tracks) and podcasts (ABS podcast shows and episodes, podcast Feeds entries) → Square, always.
Everything else → the first shape the provider declares available in the order **Landscape, Square,
Portrait**; none available → Landscape (Square for Music/podcasts) with the placeholder. Declared
availability comes from provider metadata, never from the fetched image, so the arm is stable before
the image loads: Emby `ImageTags` (`Thumb`, `Primary`) and `BackdropImageTags` (plus the series'
thumb/backdrop for episodes), which `EmbyItem` does not parse today (`api_types.rs:224`) and gains in
task 5.3; ABS books declare a portrait cover and ABS podcasts a square cover; Feeds entries declare
none (no artwork source exists). The policy returns the shape, the Emby image-type chain and the cache
key; the arm is `HeroHeader::from(artwork.shape)`, its only constructor, so no destination can choose
an arm, a chain or a key.

*Arms.* Landscape: 16:9 artwork full content width above title/meta. Portrait (2:3) and Square (1:1):
title/meta left, artwork right. One title/meta painter for all arms. Artwork shrinks before a
Workspace viewport would drop (Music's existing rule, now universal). `HeroArtworkAspect` is deleted;
its image-chain role moves into the policy.

*Artwork fit is cover (user decision):* the image fills its box and the excess is cropped, centred.
`ratatui-image` 11's `Resize::Crop` clips without scaling, so the cover step runs in the image worker:
the decoded image is `DynamicImage::resize_to_fill`-ed (`image` 0.25) to the box's pixel size and then
painted with the existing `Resize::Scale`. The box size comes from **one paint-free function**
`hero_artwork_box(area, &HeroContent) -> Rect` in the panel's arrangement module, called both by the
shell projection (to request the fetch/encode at that size) and by the panel view (to paint), so no
second layout site exists and no effect runs during paint. The projection passes the Library panel's
current area from `RootFrame`; when the box changes (resize, split drag, Workspace shrink), the next sync
pass requests a re-encode keyed by the new size and the view shows the placeholder for at most that one
frame. The Narrow inline hero (D7) is not cropped: its box takes the chosen image's own aspect.

**D6 — Workspace surface and focus are derived, not declared.**
*Removes:* `LeftPaneFocus`, `WideHeroContentBoxSurface`, and per-caller `SelectedRowSurface`.
Hero pane focusability = `hero.workspace.is_some()`; focused surface = workspace present and focused.
Workspace box surface = accent-soft while `workspace.focused`, backdrop otherwise (user decision:
Music's behaviour for all). Workspace selected row = owning surface; browser list selected row = list
backdrop — both fixed by the slot, so `SelectedRowSurface` stops being a caller argument (it stays an
internal detail of the list painter, set by the panel). ABS Book's second content box and the Podcast
episode table become the one Workspace box.

**D7 — One Narrow inline hero form.** (User decision.)
*Removes:* `CompactBannerLayout` + `render_compact_detail_with_ctx` (Movies),
`render_series_inline_detail` + `SERIES_IMAGE_COLS/ROWS` (TV), `beside_image_hero_dims` right-half
model (Home), `NarrowInlineHero`, ABS/Feeds inline painters, and every inline chapter/episode/pill
render. There is no separate Narrow hero type or producer: the Narrow skeleton derives the inline hero
from the same `HeroContent` (facts, coloured meta rows, overview, and the image the artwork policy
chose), rendering the image right-aligned at a size derived from its aspect (decoded size when cached,
the policy shape's aspect otherwise), text wrapping around it and reclaiming full width below.
Constituent lists open only via `SelectionModal` (already supports Series/Album/Podcast/Book).
**Visible change:** because the policy prefers landscape, Narrow Movies and TV show their landscape art
instead of posters, matching Wide.

**D8 — `SelectorRow` and `ListControls` are generic content.**
*Removes:* Feeds' second pill bar (`render_selector_content` closure), three pill-hit stores, Home's
private spacer paint, the home-video count label position. `SelectorRow { pills: Vec<String>, active:
Option<usize> }` always paints one bar plus the panel's spacer; the panel retains its `HitRegions`.
`ListControls { pills: Option<(Vec<String>, usize)>, label: Option<String> }` is one optional row any
destination may fill (Feeds: Watched filter pills; home videos: item-count label). Neither type has a
per-destination arm.

**D9 — Image requests leave painting.** *Removes:* every paint-time fetch: `render_card`
(`card.rs:257`), `compact_banner_layout_with_overview` (`detail.rs:173`), the Home hero
(`home_hero_emby.rs:244,261`) and `paint_music_image` (`album_art.rs:77`). The shell's projection runs
`artwork_policy` + `hero_artwork_box` for each projected hero and the queue visual slot, issues
`fetch_card_image` there (TV already does this, `push_tv_workspace_content`), and projects image state
into content. Painting reads projected state only.

**D10 — Queue column: `QueuePanel` + `QueuePlaybackPanel`; right column: `LibraryPlaybackPanel`,
`TabPanel`, `StatusBarPanel`.** (User decision: the two playback panels are distinct components.)
*Removes:* base-frame queue frame/title/status painting, the queue-only `render_player_panel` pair, the
`narrow_player` flag, shell-painted tabs and status bar. `QueuePanel` (today's `QueueComponent`) owns
its frame, title row, status pill row and list. `QueuePlaybackPanel` is mounted in every queue-visible
layout and owns the always-painted header row, the visual slot (artwork/visualizer, today's
`render_card`) and the queue-column transport presentation; while idle it paints only the header row
and its visual slot and transport take zero rows (resolves the header-vs-idle contradiction: the
header's owner is never unmounted). `RootFrame` sizes its placement from header + slot + transport
heights, and `QueuePanel` starts below it plus its separator row. It lands with the
folded change's placement rules (below 100 columns stacked; 100+ side by side, visual slot left, 2-cell
gap, panel height = max). `LibraryPlaybackPanel` (today's `PlaybackComponent`) is the strip, mounted
only when the queue column is hidden. Both call **one** width-driven transport arrangement (which rows
and indicators appear at a given width, over the seekbar/title/controls/indicator leaf Render
Components), so the strip and the queue column cannot drift in what they show at the same width; each
panel owns its own placement around it (header and visual slot in the queue column) and retains its
own transport hit geometry. `TabPanel` owns tab layout, overflow arrows and tab hit regions (emits a tab-select `Msg`
instead of `layout.tabs_hitmap`); `StatusBarPanel` owns the status row and its volume/mute/remote pill
regions. The folded change's D2–D6 and D9–D11 carry over unchanged in meaning (header row counted in
`QueuePanelInputs`, `NowPlayingStatus`, one `playback_host_label()`, header follows playback target,
strip reserved only when painted, idle collapses slot + transport, no new theme role); its D1 and D8
are reversed by this decision.

**D11 — `LayoutMain` and `LayoutPlayback` are deleted.** *Removes:* the paint-to-input side channel.
Each field moves to the component that paints it: tab hits → `TabPanel`; pill/list/hero/workspace
rects and `left_item_rows` → `LibraryPanel` and its owners' media lists; `tv_wide_*`,
`wide_music_*`, `movies_wide_right_area`, `*_area` destination fields → deleted (panel-internal);
`queue_*` → `QueuePanel`; `card` → `QueuePlaybackPanel`; transport and status pill rects → the playback
panels and `StatusBarPanel`; `panel_area`/`queue_boundary_area` → `RootFrame`. Context-menu keyboard
anchors (`selected_item_rect`, `queue_selected_item_rect`) are answered by the owning component through
the existing request path rather than a shell mirror. Consumers that only needed "which breakpoint"
read the paint-free predicate.

**D12 — TV has one content owner.** *Removes:* the Wide/Narrow component pair and its hand-off.
`TvContent` owns one series `MediaList` owner (Wide and Inline presentations over it, per
`canonical-media-lists`), the episode list, season cursor and Inline Search session; the breakpoint
changes only which skeleton paints it. `hand_off_tv_breakpoint` and the TV part of
`apply_pending_inline_search_transfer` are deleted.

**D13 — Dead paths are deleted, not migrated.** Grid (`GridMediaList`, `NarrowBrowseControl::Grid`,
`GridPaintPolicy`, the "Grid presentation" skill/AGENTS text); list-level search branches in Music and
TV painters (`render_search_box` in pill rows, `render_plain_rows` result grid); `HomeCarrier`;
`render_library`'s area publishing.

**D14 — Divergence ledger (every resolution is Emby-direction or user-decided).**

| Divergence (inventory) | Resolution |
|---|---|
| Feeds paints two pill bars | Selector row = feed groups; List controls row = Watched filter |
| Home-video count label as an extra row | List controls row label |
| Hero focus declared by caller (`LeftPaneFocus`) | Derived from Workspace presence (D6) |
| Workspace selected row: TV/Music owning surface, ABS list backdrop | Owning surface for all |
| Workspace box accent on focus: Music only | Accent-soft for all (user) |
| Pill hits in three stores; Home-only spacer | `SelectorRow` owns hits and spacer |
| ABS Book chapters in a 2nd box; Podcast episode table | One Workspace box |
| Narrow: Books render chapters inline; Podcast renders filter pills + episodes inline | Modal only (user; spec already required it for TV/Music) |
| Wide header painted 3 ways (Home/Movies, TV, Music) + ABS + Feeds | `HeroHeader` Landscape/Portrait/Square from the artwork policy (user) |
| Metadata styled per producer (spans, colours, wrapping) | Plain rows; three repeating theme colours (user) |
| Image chain + cache key per destination; Home vs tab disagree for one item | One artwork policy: Music/podcasts Square, else Landscape > Square > Portrait (user) |
| Home builds its own facts for ABS/Feeds items | One hero producer per content type, used by Home too |
| Narrow posters vs Wide landscape for the same Movie/Series | Same policy-chosen image in both |
| Overview: plain text (Home/Movies) vs box (TV) vs none (Music) | Box when present, omitted when absent (user) |
| Narrow inline hero: 3 painters, right-half 16:9 model | One right-aligned wrap-around form (user) |
| Music side-by-side-or-stacked art switch | Square header (no in-hero breakpoint) |
| Feeds `paint_feed_hero` in its own box | Policy header (podcast feeds Square, others Landscape placeholder) + overview box |
| TV two mounted components | One owner (D12) |
| Two playback presentations by `narrow_player` flag | Two distinct panels (user) |
| Non-hero Grid catalog | Deleted (no such library) |
| Music/TV list-level search branches | Deleted (unreachable) |

**D15 — Verification uses ordinary tests only.** Each slot Render Component gets buffer tests; each
mounting/focus/subscription change gets real `Application::tick()` integration tests
(`src/app/tests_tick_integration*.rs`); layout claims are role-rect containment or buffer content, never
absolute coordinates. No test compares destinations to each other as a conformance check — sameness is
a property of the types. "Every cell is painted by a panel" is checked by one tick integration test
per Panel mode that pre-fills the test buffer with a sentinel symbol, draws a frame, and asserts no
sentinel cell remains inside any mounted panel's placement (each panel fills its own surface across its
placement, D1); unwritten Ratatui cells are otherwise indistinguishable from painted blanks. Tests
that need "before the frame paints" assert the observable state after one `tick()` + sync instead,
since no pre-paint seam exists. `rg` checks named in tasks are manual confirmation, not gates. Tests that assert deleted structures (`LayoutMain` fields, per-destination
painters, Grid) are deleted with them, not ported; tests pinning a deliberately changed presentation
(D5–D8) are rewritten to the new presentation.

**D16 — Slicing: the panel types land first, destinations move one per slice, the base frame goes
last.** S0 moves draw-time mutations into sync and introduces `RootFrame` as data (still delegating
to `compose_base_frame`); S1 Tab + Status bar panels;
S2 Queue + Queue playback panels (the folded change); S3 Library playback panel; S4 `LibraryPanel`
mounted with its content types and skeletons, hosting Home; S5–S10 one destination each (Movies/home
videos/generic, Feeds, TV, Music, ABS Books, ABS Podcasts), Feeds early because it is the most
divergent test of the slot types; S11 deletes the base frame, `LayoutMain`, Grid and dead paths; S12
docs. During S4–S10 a destination not yet moved is still painted by its old mounted component inside
the library rect the root gives it — one painter per surface throughout; no surface is ever painted by
both. If a destination needs a slot or arm the content types lack, the slice stops and the type is
changed for every destination (D0).

**D17 — Base-frame element inventory: every element has one owner panel and one task.** This is the
enumeration behind "zero visible UI elements that are not components" (G1).

| Element today (painter, site) | Owner at completion | Task |
|---|---|---|
| Left column backdrop (`render_legacy_backdrops`, `chrome.rs:16`) | Queue panel + Queue playback panel fills | 3.1, 3.5, 12.1 |
| Right column backdrop (`render_legacy_backdrops`, `chrome.rs:37`) | Tab / Library / Library playback / Status bar panel fills | 12.1 (after 2.x, 4.1, 5.x) |
| Tab bar + overflow arrows (`render_tabs`, `chrome_tabs.rs:34`) | Tab panel | 2.1 |
| Status row + volume/mute/remote pills (`render_status_bar`, `chrome_status.rs:297`) | Status bar panel | 2.2 |
| Right-column transport strip (`PlaybackComponent` at `player_area`) | Library playback panel | 4.1 |
| Queue-only transport, wide + narrow (`render_player_panel`, `shell_draw.rs:276-319`) | Queue playback panel | 3.5 |
| Queue visual slot: artwork/placeholder/visualizer (`render_card`, `card.rs:257`) | Queue playback panel | 3.4, 3.5 |
| Now-playing header row (new, folded change) | Queue playback panel | 3.5 |
| Queue frame (`render_queue_panel_frame`, `widgets.rs:231`) | Queue panel | 3.1 |
| Queue status pill row (`render_queue_status`, `queue.rs:290`) | Queue panel | 3.1 |
| Queue title row + list (`QueueComponent`) | Queue panel | 3.1 |
| Queue column boundary (`QueueBoundaryComponent`, `shell_queue.rs:169`) | Queue boundary (root-placed) | 1.4 |
| Wide hero gap boundary (`WideHeroBoundaryComponent`, `shell_library.rs:363`) | Library panel | 5.9 |
| Library area publishing (`render_library`, `widgets.rs:515`) | deleted (paints nothing) | 12.1 |
| Home / Browser / TV / Music / Feeds / ABS Book / ABS Podcast bodies (component-wrapped free painters) | Library panel slots | 5.11, 6.1, 7.1, 8.2, 9.1, 10.1, 11.1 |
| Overlays, modals, popups, sidebars (`render_overlay_stack`) | unchanged: already component views | — |

## Risks / Trade-offs

- **Focus and mouse regressions from collapsing destination ids into `ComponentId::Library`** →
  every mounting change carries tick-integration tests through the real sync pass (focus follows the
  active library, mouse eligibility follows the painted panel, inactive owners keep state across tab
  changes).
- **Very large change (~40 files touched by `LayoutMain` removal alone)** → slices are ordered so each
  leaves the app shippable; `LayoutMain` fields are removed slice by slice as their consumers move, and
  the struct is deleted only when empty.
- **Visible behaviour changes users will notice** (Home/Movies overview boxed, Narrow Movies/TV image
  layout unified, ABS Narrow loses inline lists, Feeds controls row, idle queue-only panel) → all are
  user decisions recorded in D14; manual Wide/Narrow review at each slice's acceptance.
- **`LibraryPanelContent` borrows owner-held media lists mutably during view** → the owner hands
  `&mut` presentations to the panel inside its own `view` call path (`LibraryPanel::view` borrows the
  active owner and its content together); no list is copied or mirrored.
- **Slot types too narrow for a destination** → D0/D16 rule: extend the type for all destinations;
  never add a destination arm.
- **File overlap with `add-media-list-multi-select` and `add-configurable-keybinds`** → no requirement
  conflict; whichever lands second rebases. Multi-select's status-row indicator lands in
  `StatusBarPanel`.

## Migration Plan

No persisted state, protocol or config migrates. `ComponentId` is in-memory only. Rollback is reverting
the change's commits; each slice is a revertable unit. Within a slice, the move of a surface's painter is
atomic (the new painter lands in the same commit that deletes the old one), because a half-applied state
paints a surface twice. `openspec/changes/add-now-playing-sidebar/` is removed in the first commit of
this change; its requirements live in `specs/queue-playback-panel`, `panel-mode`, `idle-feed-rotation`
and `queue-only-playback` here.

## Open Questions

- Whether the queue title bar's host text is later absorbed by the header row (carried from the folded
  change). Deferrable: a later subtraction that changes nothing here.
