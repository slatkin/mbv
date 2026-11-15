# Proposal

## Why

Issue #1012 (found in the `/simplify` review of PR #1003). Every GUI component reads a
section with `sections.<x>.cloned()`. That call deep-clones the whole section value on every
render. `LibraryPanel` clones `library_list` and `library_detail`. Each destination list clones
both again. The Hero painters clone the detail a third time. `QueuePanel` clones `GuiQueue` on
every render and again in its `use_effect`. One section change costs two or more deep clones
of every row.

## What Changes

- The section signals for `library_list`, `library_detail`, `queue`, and `settings` hold a shared
  handle (`SectionRef<T>`, an `Rc<T>` newtype) in place of the value. A read clones the handle,
  not the content.
- `Sections::apply_batch` makes one `Rc` per written section. A section that the batch does not
  write keeps its handle, so a repaint never copies the content.
- `SectionRef<T>` compares by pointer. A section at one revision is immutable, so the same pointer
  means the same content. A Dioxus prop memo check costs O(1) in the size of the content.
- Components borrow from the handle (`as_deref()`, `library_pair`) or hold the handle. They never
  copy rows out of it.
- `LibraryPanel`, the destination lists, the Hero painters, `QueuePanel` (render and effect), and
  `settings_section_view` stop deep-cloning.
- No change to what the window paints, to the section batch, or to the Client core.

**Non-goals**

- `tabs` and `pills` stay cloned. Their size follows the tab and pill count, not the row count.
- No bench or measurement step. The user decided to skip it.
- No change to the `mbv-gui` core thread, the channel, or `GuiSectionBatch` (it stays `Send`).
- No other section (`playback`, `status`, `overlay`, ...) changes: they are small.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: adds one requirement. The window reads a section without copying its content
  during a repaint, and a repaint that carries no new section revision allocates no new section
  value.

## Impact

- `crates/mbv-gui/src/window/sections.rs` (+ `sections/` for `SectionRef`): the signal types
  and `write`.
- Readers of the four sections: `window/app.rs`, `window/components/{library_panel,library_list,
  library_emby_list,library_audiobooks_list,library_feeds_list,library_podcasts_list,
  library_shows_list,library_home_list,library_hero,queue_panel,settings_screen}.rs`,
  `window/library/**` (`peek()` readers), `window/settings/state.rs`, and their tests.
- No new dependency. `Rc` is allowed in the single-threaded window: the user approved it for the
  section signals (it is an exception to the "prefer owned data" rule in `AGENTS.md`).
- No change to `docs/adr/`. `docs/architecture/gui-client.md` gets one sentence on the handle.
