## Why

Issue #607's acceptance criterion is "component-local interaction state has one
owner." After #615–#618 and the two sibling changes
(`split-audiobookshelf-cursor-ownership`,
`remove-music-workspace-cursor-mirror`), no *echo loop* remains: each value has
one writer per direction. But the criterion is still unmet, because the values
themselves still live in `App`:

- `BrowseLevel` (`src/app/types_browse.rs`) holds `cursor` and `scroll`
  alongside `items`, `total_count`, `loading`, `all_items`, `sort_by`,
  `letter_filter`, and `music_grouping`.
- `AudiobookshelfBrowseState` holds `selected_id`, `episode_selection`, and
  `scroll` alongside `shows`, `detail_cache`, and `progress`.
- `AudiobookshelfBookBrowseState` holds `selected_id`, `chapter_selection`, and
  `selected_bucket` alongside `books`, `detail_cache`, and `buckets`.

`remove-browser-cursor-scroll-mirror` deliberately deferred this: *"No
`BrowseLevel` field is deleted (see scout handoff §5 for the ~37 unrelated
readers that block it)."* That deferral is the whole of the remaining work.

One struct carrying both content and interaction state is what forces every
remaining workaround in the tree:

- The components hold a **clone of the shell's own struct**, so every
  projection is a clobber-then-restore of the interaction fields
  (`audiobookshelf_podcast.rs:61-85`, `audiobookshelf_book.rs:60-77`).
- `App::apply_lib_cursor_index` exists only to write a component-resolved
  cursor back into a shell field so ~37 readers can find it.
- The rule that a `sync_*` must not mirror interaction state back into `App`
  is enforced by review and an ast-grep rule, because the type system cannot
  express it while the fields are on the shared struct.

AGENTS.md states the standard this violates directly: *"NEVER store the same
fact twice (a field mirroring a derived value)"* and *"If you catch yourself
writing a comment warning the next reader not to do something, change the type
so they can't."* The tree currently has several such comments.

There is no user-visible payoff. This is tech debt paid off while the migration
context is still live, and the last item standing between #611 and closure.

### Addendum (2026-08-29) — the narrow breakpoint, and three live regressions

Task 4.4's R14 constraint fired as designed: `library_list_render_ctx` cannot
read the live cursor from a component, because for narrow TV **no component is
mounted at all**. Tracing that gap found it is also the cause of three
regressions already visible in the app, all one root cause — the migration
gated legacy `render_list` off for each *wide* surface as it landed and never
addressed the narrow breakpoint:

1. **Narrow Movies double-paints.** Legacy `render_list` and
   `BrowserComponent::view` both paint `layout.main.left_area`. Latent until
   `6cf469e1` (#618) removed the cursor mirror that kept the two in step; now
   the cursors diverge, giving doubled rows and a stale-cursor inline hero.
2. **Narrow TV navigation is dead.** No component owns the surface, and legacy
   browse key handling was deleted in `51bb3a16`, so keys reach the router,
   find no focused component, and fall through to nothing.
3. **Narrow grouped Music's painted cursor is frozen.**
   `MusicWorkspaceComponent` is mounted and owns the cursor, but paints into
   `wide_music_area`, which is empty at narrow — so keys move a cursor the
   legacy painter never reads.

Closing that gap is user-visible work with its own payoff, so it was split out
rather than absorbed here (maintainer decision, 2026-08-29). This change now
**ends at task 4.4**, and the rest becomes a dependency chain:

| Change | Scope |
|---|---|
| `migrate-narrow-browse-to-components` | mount the missing components, hoist `render_list`'s narrow composition into them, land the R14 threading. Closes the three regressions |
| `delete-browse-level-cursor-scroll` | the old 4.5 + phases 5 and 6 — field deletion, `apply_lib_cursor_index`, the movers, the mouse paths, the ast-grep clauses |
| `sync-interactive-surface-docs` | the old phase 7 — ledger, ADR 0022, spec, and #607's acceptance check |

The scope statement above stands as written; only the *ending* moves. What this
change delivers is the two Audiobookshelf struct splits, the resting-position
type, and every outcome-1/2/3 re-point except R14.

## What Changes

- Separate content from interaction state in all three structs. Content is what
  the shell computes and projects; interaction state is what the component
  owns. A component receives only the content half, so a projection can no
  longer overwrite an interaction value and no restore dance is needed.
- Distinguish the two things `BrowseLevel.cursor` currently conflates:
  - the **live cursor**, owned by the component, never in `App`;
  - the **resting position** for a non-visible level, owned by `App` because it
    is persisted and restored (`LibraryPosition`, `save_default_library_position`).
  These are the same field today, which is why "is this read a mirror?" has no
  mechanical answer.
- Reclassify every reader of the deleted fields into one of three outcomes:
  take the resolved value as a parameter (the pattern
  `remove-tv-workspace-cursor-mirror` established with
  `activate_selected_series_item`), read the resting position, or read the
  component. Task 1 produces the authoritative inventory.
- Retire `App::apply_lib_cursor_index` and the delta movers left unreachable —
  `move_lib_cursor_rows` and `jump_lib_cursor` already have no live caller, and
  `move_lib_cursor` is reachable only from `mouse_gestures.rs`, which is
  accepted-broken under D16.
- Give every narrow browse surface a mounted component that owns its cursor,
  scroll, and keys (Phase 4A) — the precondition for deleting
  `BrowseLevel::cursor`, and the fix for the three regressions above.
- Remove the ast-grep rule clauses that policed by convention what the types
  now make unrepresentable, keeping the ones that still guard a real boundary.

## Non-goals

- No change to the on-disk `LibraryPosition` shape, shared-document format, or
  ctrl protocol. Persistence keeps saving a cursor and scroll; only the
  in-memory owner of the *live* value changes.
- No repair of mouse routing (D16). Where `mouse_gestures.rs` is the sole
  caller of something being deleted, delete the caller with it rather than
  keeping the function alive to serve it.
- No behavioural change to browsing, playback, persistence, or restore, beyond
  closing the three regressions named in the addendum. Every task carries a
  characterization test taken before the change.
- No migration of the narrow painters into components (design.md D7). Legacy
  `render_list` stays the narrow painter; only ownership of cursor, scroll,
  and keys moves.

## Capabilities

### Modified Capabilities
- `interactive-component-framework`: adds a structural requirement — a type
  projected from shell to component carries no field the component owns — and
  names the live-cursor / resting-position distinction the framework has so
  far left implicit.

## Impact

- `src/app/types_browse.rs`, `src/app/types_audiobookshelf_browse.rs`, and
  every reader task 1 identifies (#618's scout recorded ~37 non-test
  `BrowseLevel` readers; the ABS structs add their own).
- `src/app/lib_cursor_actions.rs`, `src/app/mouse_gestures.rs`,
  `src/app/actions_navigation.rs`, `src/app/music_grouping.rs`,
  `src/app/music_actions.rs`, `src/app/context_menu_actions.rs`,
  `src/app/library_search_actions.rs`, `src/app/library_position_state.rs`,
  `src/app/browse_level_actions.rs`, plus the corresponding components and
  shell projection sites.
- `rules/interactive-component-boundary/`, `openspec/specs/`, ADR 0022, and
  `docs/architecture/interactive-surface-ledger.md`.
- Expect several files to cross the 800-line cap during the migration; split
  them in the same PR per AGENTS.md.

## Sequencing

Land after both sibling changes. Their round-trip removals are what make the
reader classification in task 1 tractable — attempting this first means
classifying readers whose call graph still contains a recompute step.
