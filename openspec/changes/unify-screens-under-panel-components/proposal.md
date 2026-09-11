## Why

**This change exists to unify the screens.** The same screen is built five different ways, and every
earlier attempt at unification added a helper, a variant or a conformance test instead of removing the
difference (#691). The TuiRealm migration moved *state* into components but left *composition* in
pre-TuiRealm free painters: the shell still paints a legacy base frame (`compose_base_frame`,
`paint_legacy_chrome`, `render_legacy_backdrops`), and each destination's component `view()` builds a
throwaway `LayoutMain` and calls a per-destination painter that lays out the whole pane. Because each
destination builds its own screen, a shared rule is only as strong as each caller's willingness to
call it: Feeds paints two pill bars, Music alone accents its track box, ABS Books alone renders chapters
inside the Narrow inline hero, and the Wide hero header is painted three ways.

The fix is structural. Sameness must come from types and ownership, not from enforcement outside the
code: a parent panel component owns the skeleton, and a destination can only supply typed content
into slots the panel defines. If the panel's content type has no place for something, no destination
can paint it. **Done means zero visible UI elements that are not components**, and no destination that
can look different from its siblings except through the shape of its content.

## What Changes

- **Root composes the frame from panel components.** The root composes Tab panel, Library panel,
  Library playback panel, Queue panel, Queue playback panel and Status bar panel directly; the legacy
  base frame is deleted: `compose_base_frame`, `render_main`, `paint_legacy_chrome`,
  `render_legacy_backdrops`, the hand-ordered `render_*_component` list in `shell_run.rs`, and
  `LayoutMain` as a shell-wide paint-and-hit struct. State mutations that `render_main` performs today
  move into the shell's sync pass.
- **One Library panel skeleton, two breakpoints.** A Wide and a Narrow library panel own every slot:
  Selector row (exactly one pill bar), List controls row (one universal, optional row), list box
  (Main content box framing plus the canonical media list, empty state, or Inline Search), and in Wide
  the Hero pane (Hero header, optional overview Main content box, optional Workspace). Home, Movies,
  home videos, TV, Music, ABS Books, ABS Podcasts and Feeds supply typed content only; none of them
  lays out the panel.
- **Emby screens are the reference model.** Where destinations differ, ABS and Feeds conform to Emby.
  Where two Emby screens differ, the decision is recorded once in `design.md` and applied everywhere.
- **Three Wide Hero header types**, chosen by the item's kind and never by the caller: Landscape
  (artwork above the text, full pane width), Portrait and Square (title and metadata on the left,
  artwork on the right). The type is the shape of the artwork one policy chooses for every item: Music
  and podcasts always Square, everything else the first available of Landscape, Square, Portrait. One
  hero producer per content type is used everywhere, Home included, so an item has the same facts,
  image and header on every screen, Wide and Narrow. Metadata is plain rows coloured by the panel with
  three repeating colours. Artwork always fills its box, cropped centred when its aspect differs.
  **BREAKING (visual)**: Narrow Movies and TV show landscape art instead of posters. The overview Main content box is the same for all three and is omitted when
  the item has no overview. **BREAKING (visual)**: Home and Movies overviews move into the box;
  Music's side-by-side-or-stacked switch is replaced by the Square header.
- **One Narrow inline hero form**: the content's image right-aligned with title, metadata and overview
  wrapping around it, for every destination. **BREAKING (visual)**: the 16:9 right-half meta-column
  model is removed.
- **Workspace lists are uniform.** Every Wide Workspace is an optional Selector row plus one list in a
  Main content box whose surface turns accent-soft while its list holds focus (today Music only), with
  the owning-surface selected row (today TV/Music only). In Narrow, workspace lists open only through
  the shared selection modal. **BREAKING**: ABS Books stops rendering chapters inside the Narrow inline
  hero; ABS Podcasts stops rendering filter pills and episode rows there.
- **Feeds**: its second pill bar is deleted; the Watched filter moves to the List controls row.
- **Deleted as unreachable**: the Grid presentation and the non-hero two-column catalog (every library
  in use has a hero); list-level search branches that no producer can reach (Inline Search is the only
  live search and becomes a Browser pane state).
- **Image fetching leaves painting**: queue artwork and Narrow poster fetches move from paint
  (`render_card`, `compact_banner_layout_with_overview`) into the shell's projection pass.
- **Folds in `add-now-playing-sidebar`** (whose change directory is removed): the queue column gains an
  always-visible status/target header row, the transport renders in the queue column in every
  queue-visible layout, idle collapses the visual slot and transport, and the right-column strip
  renders exactly when the queue column is hidden. Its "sidebar" is renamed **Queue playback panel**
  (the glossary's Sidebar term is taken), and its D1 is reversed: the Queue playback panel is a
  component, not a base-frame composition. The Library playback panel (strip) and Queue playback
  panel are two distinct components; only one is mounted per frame. **BREAKING (behaviour)**: a
  connected but idle transport no longer keeps a panel in queue-only.
- **Docs**: `CONTEXT.md` broadens *Panel* to every root-composed region, renames Panel mode *Normal*
  to *Narrow*, and adds the slot terms; ADRs 0022–0024 state composition ownership; the
  `mbv-frontend` skill and interactive-surface ledger stop claiming the migration is complete and stop
  endorsing caller-selected variants.

No tests, `ast-grep` rules, scripts or CI gates are added as enforcement of sameness; the types are the
enforcement. Ordinary buffer tests and `Application::tick()` integration tests prove behaviour.

## Capabilities

### New Capabilities

- `library-panel`: the Wide and Narrow library panel skeleton, its slots, the three Wide Hero header
  types, the Workspace, the Narrow inline hero form, and the rule that destinations supply only typed
  slot content with Emby as the reference model.
- `queue-playback-panel`: the queue column's header row, visual slot and transport, their placement in
  every queue-visible layout, idle collapse, pointer input, and where the Library playback panel
  renders instead (re-homed from the removed `add-now-playing-sidebar` change).

### Modified Capabilities

- `interactive-component-framework`: adds composition ownership — the root composes every visible
  surface from panel components with no legacy base frame, panels absent from a Panel mode are
  unmounted, and no component view builds shell-wide geometry or calls a per-destination free painter;
  the existing completion definition is modified so a ledger of state ownership alone no longer counts
  as complete.
- `hero-big-text-title`: retired; the Octant BigText title was never implemented and the unified Hero
  header uses one title presentation.
- `right-panel-arrangements`: the overview box is omitted when empty; the Narrow inline hero has one
  image model; per-screen presentation declarations, the Feeds "preserve restore-feeds" requirement
  and the non-hero two-column carve-out are removed.
- `canonical-media-lists`: removes the Grid presentation and the non-hero catalog flow.
- `ui-design-system`: structural differences come only from content shape; caller-selected variants
  with a single user are defects, not approved vocabulary.
- `audiobookshelf-book-browsing`: Narrow selected-book detail no longer contains chapters; chapters
  open through the selection modal.
- `audiobookshelf-podcast-library-ui`: Narrow selected-show detail no longer contains filter pills or
  episode rows, and podcast-specific presentation declarations are removed.
- `feeds-service-wide-list`: the Narrow-preservation requirement is removed (Narrow Feeds changes to
  one Selector row plus the List controls row).
- `panel-mode`: library-only renders the Library playback panel; queue-only renders the Queue playback
  panel (re-homed from the folded change).
- `idle-feed-rotation`: display and the open-link command follow the playback panel's presence
  (re-homed from the folded change).
- `queue-only-playback`: retired; superseded by `queue-playback-panel` (re-homed from the folded
  change).

## Impact

- Shell: `src/app/shell_run.rs`, `shell_draw.rs`, `shell_library.rs`, `shell_queue.rs`,
  `shell_playback.rs`, every `shell_<destination>.rs` sync/push/render trio, `shell.rs` and `input*`
  readers of `app.layout`, `src/app/layout.rs` (`LayoutMain`, `LayoutPlayback`,
  `FrameChromeGeometry`).
- Components: `src/app/components/` gains the panel components and slot components; destination
  components (`home.rs`, `browser/`, `tv_workspace/`, `music_workspace.rs`, `feeds.rs`,
  `audiobookshelf_book.rs`, `audiobookshelf_podcast.rs`, `queue.rs`, `playback.rs`, the two boundary
  components) lose their painters and become content producers inside the Library panel.
- Render: `render/arrangements/wide_hero.rs`, `library.rs`, `chrome.rs`, `queue.rs`; per-destination
  painters in `render/components/` (`tv_wide.rs`, `music_wide*.rs`, `feeds.rs`,
  `audiobookshelf_*.rs`, `home*.rs`, `list*.rs`, `detail*.rs`, `card.rs`, `chrome*.rs`, `widgets.rs`)
  are replaced by slot Render Components; `ListChromeVariant`-style, `LeftPaneFocus`,
  `WideHeroContentBoxSurface`, `NarrowInlineHero`, `CompactBannerLayout` are deleted or replaced by
  content types; `HeroArtworkAspect` becomes a function of the Hero header type.
- Image worker: Wide header artwork is cover-cropped to its box (`image` `resize_to_fill`) before
  encoding; no new dependency.
- Implementation base is `main`; the unmerged `refactor/unify-wide-hero-content-box-frame` branch is
  discarded.
- Docs: `CONTEXT.md`, `docs/adr/0022`–`0024`, `docs/architecture/interactive-surface-ledger.md`,
  `.agents/skills/mbv-frontend/SKILL.md`, `AGENTS.md` repository map.
- `openspec/changes/add-now-playing-sidebar/` is removed; its requirements live here.
- No protocol, daemon, provider, config or dependency changes. File overlap with in-flight
  `add-media-list-multi-select` (list rows, status row) and `add-configurable-keybinds` (router);
  `unify-wide-hero-content-box-frame` is superseded and left untouched for the user to delete.
