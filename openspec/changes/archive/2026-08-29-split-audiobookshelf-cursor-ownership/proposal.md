## Why

Issue #611's survey recorded the Audiobookshelf book browser as already
migrated ("carries no pin") and the Audiobookshelf podcast browser as a
"trailing cleanup" folded into #616. Both readings are wrong in the same way
the issue's own *Corrections* section already documented once: status was
inferred from a projection call site without tracing the round trip.

Both surfaces carry the exact duplicate-arithmetic round trip that
`remove-browser-cursor-scroll-mirror` eliminated for the Emby browser, and
that the `interactive-component-framework` spec already forbids in its
"A local movement that also persists carries its resolved value once"
scenario. This change is therefore a compliance fix against an existing
requirement, not a new capability.

The shape, identical on both surfaces:

1. The component resolves the movement locally
   (`AudiobookshelfPodcastComponent::move_cursor`,
   `src/app/components/audiobookshelf_podcast.rs:140`;
   `AudiobookshelfBookComponent`'s `state.select(...)` / bucket arithmetic,
   `src/app/components/audiobookshelf_book.rs:254-302`).
2. It emits a **delta** (`PodcastShowMove`'s 8 variants,
   `AudiobookshelfBookMove`'s 12 variants).
3. The shell independently recomputes the same movement against `App`
   (`shell_messages.rs:217-250` → `App::move_audiobookshelf_show_cursor` /
   `_rows` / `jump_…`; `shell_audiobookshelf_book.rs:14-45` →
   `App::move_audiobookshelf_book_cursor` / `_row` / `jump_…` /
   `cycle_audiobookshelf_book_bucket`).
4. The next `push_*` overwrites the component's result with the App's.

Two concrete defects follow from it, not just architectural debt:

- **Page-size divergence.** The component pages by its own painted
  `page_size()`; the shell pages by `App::lib_page_size()`. When the two
  disagree the component paints one cursor and the push corrects it to
  another, producing a visible jump.
- **Stale-value adoption.** Both `set_content` implementations clobber the
  whole state with the App snapshot and then restore the component-owned
  fields **only if** the previously selected id survived
  (`audiobookshelf_podcast.rs:61-85`, `audiobookshelf_book.rs:60-77`). When it
  did not, the component silently adopts `App`'s stale `episode_selection`,
  `scroll`, and `selected_bucket` instead of resetting them.

## What Changes

- Replace `PodcastShowMove` (8 delta variants) with a single request carrying
  the component-resolved show index, and add an `App` entry point that applies
  that index directly while preserving the existing position-save and
  detail-fetch tail — the same treatment `App::apply_lib_cursor_index`
  (`src/app/lib_cursor_actions.rs:241`) gave the Emby browser.
- Replace `AudiobookshelfBookMove` (12 delta variants) with resolved-value
  requests for the four distinct movements it actually encodes: book index,
  chapter selection, bucket index, and pane focus transition.
- Delete the `App` movement helpers left unreachable by the above
  (`move_audiobookshelf_show_cursor`, `move_audiobookshelf_show_rows`,
  `jump_audiobookshelf_show_cursor`, `move_audiobookshelf_book_cursor`,
  `move_audiobookshelf_book_row`, `jump_audiobookshelf_book_cursor`,
  `cycle_audiobookshelf_book_bucket`, and the book focus helpers), keeping
  only what a live non-mouse caller still needs.
- Make the `set_content` restore of component-owned interaction fields
  unconditional: when the previously selected id is gone, reset those fields
  to their own defaults rather than falling through to the App snapshot's
  values, and clamp indices against the new content.
- Give paging one page-size source, so the component's local resolution and
  any shell-side clamp cannot disagree.

## Non-goals

- **No `AudiobookshelfBrowseState` / `AudiobookshelfBookBrowseState` field is
  deleted.** The components hold a clone of the shell's own struct, so
  `selected_id`, `episode_selection`, `scroll`, `chapter_selection`, and
  `selected_bucket` cannot move out of it while that struct is the projection
  payload. Separating content from interaction state is
  `split-browse-state-interaction-fields`, and this change must not pre-empt it.
- No change to the podcast episode-transition or book playback/enqueue
  intents, the cover-fetch bridge, or mount lifecycle.
- Mouse remains accepted-broken (D16). Do not repair or re-route
  `mouse_gestures.rs`.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `interactive-component-framework`: the existing resolved-value requirement
  gains an Audiobookshelf scenario and a scenario closing the stale-adoption
  hole in conditional projection restores.

## Impact

- `src/app/components/msg/intents.rs`, `src/app/components/msg/shell.rs`,
  `src/app/components/audiobookshelf_podcast.rs`,
  `src/app/components/audiobookshelf_book.rs`,
  `src/app/shell_messages.rs`, `src/app/shell_audiobookshelf_podcast.rs`,
  `src/app/shell_audiobookshelf_book.rs`,
  `src/app/audiobookshelf_browse_actions.rs`, and the component/shell test
  modules for both surfaces.
- No persistence format, ctrl protocol, or provider API change.
- `docs/architecture/interactive-surface-ledger.md`: the two Audiobookshelf
  rows record the round trip's removal.
