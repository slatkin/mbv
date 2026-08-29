## Context

Both Audiobookshelf browsers run the duplicate-arithmetic round trip that
`remove-browser-cursor-scroll-mirror` removed from the Emby browser. The
favourable discovery is that **index-taking entry points with the full effect
tail already exist on both surfaces**, so this change is mostly a matter of
routing to them and deleting the delta wrappers that sit on top:

| Surface | Existing index-taking entry point | Effect tail it already runs |
|---|---|---|
| Podcast shows | `App::select_audiobookshelf_show(cursor)` (`audiobookshelf_browse_actions.rs:152`) | clamp, `state.select`, `start_audiobookshelf_detail` |
| Book list | `App::select_audiobookshelf_book(cursor)` (`:359`) | clamp, `state.select`, `save_audiobookshelf_book_position`, `start_audiobookshelf_book_detail` |
| Book buckets | `App::select_audiobookshelf_book_bucket(bucket_pos)` (`:465`) | bucket narrowing, cursor re-anchor |

The delta wrappers (`move_audiobookshelf_show_cursor` / `_rows` /
`jump_audiobookshelf_show_cursor`, `move_audiobookshelf_book_cursor` /
`jump_audiobookshelf_book_cursor` / `cycle_audiobookshelf_book_bucket`) exist
only to recompute, from `App`, arithmetic the component has already done.

## Decisions

### D1 — Requests carry a resolved index, not a delta

`PodcastShowMove`'s 8 variants collapse to one request carrying the
component-resolved show index. `AudiobookshelfBookMove`'s 12 variants collapse
to three resolved-value requests (book index, bucket position, chapter
selection) plus nothing else — the pane-focus variants are folded into D3.

*Rationale:* this is the shape the `interactive-component-framework` spec
already mandates ("carry the resolved value in the `Msg`; the shell SHALL
apply that value directly rather than independently recomputing"). It also
removes the page-size divergence: only the component ever pages, so
`App::lib_page_size()` stops being a second, competing page stride for these
two surfaces.

*Rejected:* keeping deltas and making the shell authoritative (i.e. deleting
the component's local movement). That inverts the ownership the ledger
records for these rows and re-introduces a full-round-trip latency on every
arrow key.

### D2 — The podcast episode-selection guard moves to the component

`move_audiobookshelf_show_cursor` currently refuses to move while
`state.episode_selection.is_some()` (`:178`). The component already applies
the identical guard as a `handle_key` match guard
(`audiobookshelf_podcast.rs:155-200`, `if self.state.episode_selection.is_none()`),
so once the delta wrapper is gone the guard exists exactly once, on the side
that owns the value. `select_audiobookshelf_show` must not grow a copy of it.

### D3 — Chapter focus is one `Option<usize>` setter, not three verbs

`focus_audiobookshelf_book_chapters` (sets `Some(0)`),
`focus_audiobookshelf_book_browser` (sets `None`), and
`move_audiobookshelf_book_row(delta)` (clamped delta on the same field) are
three verbs over one field. The component owns chapter focus and resolves the
target itself, so they collapse into a single shell entry point taking the
resolved `Option<usize>`.

The `chapter_selection.is_some()` precondition inside
`focus_audiobookshelf_book_chapters` (`selected_id.is_some() && chapter_selection.is_none()`)
is a *transition* guard, not a persistence effect — it belongs with the
component's `chapters_visible` geometry check, which already gates the same
transition on rendered layout (`audiobookshelf_book.rs:27-30`).

### D4 — Projection restore is unconditional; content changes clamp

Today both `set_content` implementations save the component-owned fields,
overwrite everything with the App snapshot, and restore the saved fields
**only inside** an `if previously-selected-id-still-present` branch. When the
selection is gone the component keeps the App snapshot's values for
`episode_selection`, `scroll`, and `selected_bucket` — values `App` has no
business owning.

The restore becomes unconditional in the sense that the component's own values
always win over the snapshot's; when the selected id is gone, the component
resets *its own* fields to their defaults and clamps surviving indices against
the new content. `App`'s copies are never adopted.

*Rationale:* this is the smallest change that makes the two structs' shared
interaction fields harmless while they still exist. It is deliberately a
behavioural guard rather than a type change — the type fix is
`split-browse-state-interaction-fields`, and doing it here would make this
change unreviewable.

### D5 — Field deletion is explicitly out of scope

`AudiobookshelfBrowseState` and `AudiobookshelfBookBrowseState` are both the
shell's state struct *and* the component's own state struct (the component
holds a clone). No interaction field can leave them until that is split. This
change must leave every field in place; a diff that deletes one has taken
`split-browse-state-interaction-fields`'s work and should be rejected.

## Risks

- **Silent behavioural drift in paging.** The component's `page_size()` and
  `App::lib_page_size()` may already disagree today, meaning current
  behaviour is the *App's* stride and users are used to it. Task 1 pins the
  present behaviour with a characterization test before anything moves, so a
  stride change is a visible test failure rather than a silent regression.
- **`state.select()` side effects.** `AudiobookshelfBrowseState::select` also
  resets `episode_selection` (`types_audiobookshelf_browse.rs:116`). Routing
  component movement through `select_audiobookshelf_show` must preserve that
  reset, or entering a show then moving away leaves episode mode armed.
