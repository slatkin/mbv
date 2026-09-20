# Handoff: Grouped Music tree browser (tui-treelistview evaluation)

Status: exploration only. No code, specs, or OpenSpec artifacts were changed.
Source: `/opsx:explore` session, continued after the prior agent's context loss
(transcript preserved in `planning.html`).

## Goal

A **mergeable one-screen vertical slice** that rebuilds Grouped Music's album
browser as a shallow tree using
[`tui-treelistview`](https://github.com/hexqnt/tui-treelistview) (Ratatui 0.30,
matching this repo's `ratatui = "0.30"`).

This is not a throwaway PoC. Visual design and customization quality are
acceptance criteria. **Rejecting the dependency is a valid outcome** if its
extension points cannot reproduce mbv's row treatment; building a parallel
bespoke tree renderer instead would defeat the evaluation and needs a separate
decision.

## Confirmed product decisions

- Scope is **Grouped Music only**. Queue, Home, Workspaces, other destinations'
  Inline Search, and every other library list stay canonical flat `MediaList`s.
- The browser is a two-level tree: artist = focusable root, album = leaf.
- Artist roots:
  - expand/collapse;
  - support play, enqueue, shuffle, and context actions;
  - select their visible descendant albums for multi-selection;
  - display aggregate tri-state selection.
- Fuzzy filtering:
  - reuses mbv's case-insensitive `SkimMatcherV2` and 300 ms debounce
    (`src/app/components/inline_search.rs`);
  - matches album leaves against composite text: artist + album title + year;
  - searches only the current settled Grouped Music tree — no async
    full-library fetch;
  - preserves settled artist and album ordering (score is a predicate, never a
    sort key, in this slice);
  - retains matching ancestors and force-expands matching paths;
  - shows the full tree for an empty query (this differs from today's Inline
    Search, which shows no rows on empty query);
  - restores pre-filter selection and persistent expansion on dismissal.
- While a filter is active, an artist node *means* exactly its visible matching
  albums:
  - artist actions affect only those albums;
  - artist multi-selection includes only those albums;
  - the artist Workspace shows tracks only from those albums.
- Album focus keeps today's album Hero and album-track Workspace behaviour.
- Artist focus shows artist artwork, a text summary (album count, year span),
  and all in-scope tracks grouped by album in the Workspace.
- The artist Workspace stays a canonical flat `MediaList` — album headings
  non-selectable, track leaves = stable Emby track IDs. It is not a second tree.

## Intended interaction

```text
Up/Down, j/k     Move across artist and album nodes
Right            Expand artist
Left             Collapse artist, or move album focus to parent
Enter on artist  Toggle expansion
Enter on album   Existing album/track behaviour
Ctrl+P/A/S       Play / enqueue / shuffle focused artist or album
.                Context menu for focused artist, album, or selection
V, Ctrl+Click    Select visible descendant albums
Shift+Click      Extend over visible album leaves, skipping roots
/                Open in-place fuzzy filter (tree stays behind it)
Esc              Clear filter first, then ordinary dismissal precedence
```

## Architecture

```text
GroupedAlbumCatalog  (src/app/music_grouping.rs)
        |
        v
MusicTreeModel
+-- Artist node: stable artist identity (see risk 1)
|     +-- Album node: existing stable album target
        |
        v
TreeListViewState<NodeId>      <- sole cursor, expansion, scroll,
        |                         marks, projection cache
        v
TreeQuery<MusicFuzzyFilter, NoSort>
        |
        v
TreeListView  +  TreeLabelRenderer / TreeColumnSet / TreeListViewStyle
```

Boundaries:

- `MusicContent` remains the event boundary; components never see Services,
  `App`, or `PlayerProxy`.
- The crate's `keymap` feature stays **off**. Keys arrive through
  `src/app/router.rs` / `key_policy.rs` / `input_resolver.rs` and are mapped to
  `TreeViewAction` by hand. No second router.
- `NodeId` is a compact `Copy + Eq + Hash` index into a persistent identity
  arena mapping to either an artist key or an existing album target (the crate
  requires `Copy` IDs; mbv targets are commonly `String`). IDs must survive a
  settled-catalog refresh or selection and expansion will jump.
- Spacers stop being model rows; the painter derives root separation.
- No cursor/scroll mirror in `App`; no second cursor beside
  `TreeListViewState`.
- Hit testing comes only from the latest completed render (ADR 0024).
- No generic `TreeMediaList` abstraction in this slice.

## Acceptance gates

Behaviour:
- Artist roots and album leaves are keyboard- and mouse-focusable; activation
  and context behaviour is unambiguous.
- Artist actions resolve to ordered album descendants in settled display order
  (filtered scope while a query is active); artist IDs never reach shell
  effects — only ordered album identities do.
- Expansion, selection, and viewport survive ordinary settled-catalog refresh.
- Collapsing a root removes its albums without breaking scroll or hit geometry.
- Album leaves still drive album Hero, tracks, artwork prefetch, and cursor
  persistence.
- Wide, Narrow, Mini, and Library Hero overlay presentations stay coherent, one
  owner and one painter per surface per breakpoint.

Visual / customization (user review is the final gate):
- Indentation and expand/collapse glyphs fit mbv's design language.
- Artist and album rows are visibly distinct without raw destination colours.
- Focused selection uses the canonical full-row bar; selected-row background
  follows the grandparent-panel rule.
- Zebra/group treatment, year metadata, truncation/marquee, scrollbar, and
  narrow widths are acceptable at representative Wide and non-Wide sizes.
- Live review happens after automated checks, and can reject the dependency.

## Open risks

1. **No stable artist identity exists today.** `api_types.rs:266` keeps
   `artist: String` only; album artwork and tracks are cached by album ID. Artist
   artwork and a one-request artist track fetch both need a retained Emby
   `MusicArtist` ID plumbed through from the album/track payload. Name-only
   lookup is unsafe when two artists share a display name. A deterministic
   synthetic key is still needed for `"Unknown Artist"` and servers that omit
   artist metadata; those groups get no artwork request.
2. **Group identity is not the heading label.** Letter grouping can emit two
   distinct `#` headings (numeric/symbol before `A`, accented after `Z`).
   Parents need a stable semantic key.
3. **Artist track loading must be lazy and generation-keyed.** Fetch once on
   artist focus, cache by artist ID, discard completions stale by focus or
   settled-tree revision — never paint tracks under the wrong artist. Do not
   preload every artist.
4. **The crate's marks are not mbv multi-selection.** It will not expand a
   marked parent into marked descendants; mbv materializes the ordered album
   slice itself.
5. **The stock `Table` painter probably cannot carry mbv's row semantics.** The
   projection/state engine is the attractive part. If `TreeLabelRenderer`,
   `TreeColumnSet`, and `TreeListViewStyle` fall short, acceptance fails.

## Spec conflicts a proposal must reconcile

Current specs state that headings are non-selectable, that Grouped Music uses
the canonical `MediaList`, and that Inline Search replaces the list with flat
results. The active `group-aware-page-navigation` change also assumes headings
are structural and skipped.

- `canonical-media-lists`
- `stable-music-library-grouping`
- `music-library-hero`
- `inline-library-search`
- `media-list-multi-select`
- active change `group-aware-page-navigation`

## Next step

Scaffold the change with `openspec new change "<name>"` and write
proposal/design/specs/tasks from the decisions above. Not yet done — needs your
go-ahead.
