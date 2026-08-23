## Context

Hero-on-left already shares its shell (`shared_hero_presentation`,
`hero_on_left_right_pane`, `hero_on_left_list_panel_border`,
`hero_on_left_recessed_box`, `render_pill_bar`). The design language is one
layout; only the inset box's contents vary by type. Two concrete drifts remain,
plus one scope cleanup. Selection background is out of scope — see
`unify-selected-row-background`, which lands first.

## Decisions

### Podcasts wide reuses the book-browser shape

`render_audiobookshelf_podcasts`' wide branch paints the show list straight onto
`right_panel`. Change it to mirror `render_audiobookshelf_book_right_pane_wide`:
call `hero_on_left_right_pane`, render `render_audiobookshelf_podcast_bucket_pills`
into `pills_area`, render the show list into `list_panel`. The bucket-pill fn
already exists (used by narrow) — no new pill logic. Delete the
"Wide mode has no equivalent row" comment; it rationalized the drift.

### Emby podcast libraries are generic

Remove `is_podcast_library` from the arrangement/detail branches
(`list.rs:136`, `list.rs:241`/`:263`, `detail.rs:94`/`:343`) so an Emby podcast
library falls through to the same path a generic library of its shape takes.
Assess `feed_actions.rs:386` `is_podcast_library` itself: if nothing else depends
on it after the branches are removed, delete it (clippy will confirm dead). If the
Feeds side still uses it, leave the fn and only drop the render-side callers.

### Inset-box framing is asserted, not rebuilt

No code change is required for "one layout, contents vary" beyond the two above —
it is already true. The spec delta records it so future surfaces supply inset-box
contents only.

## Risks

- Emby podcast libraries currently route to `render_wide_tv`; after removal they
  route by generic rules. Confirm the resulting arrangement is the intended
  generic one (movies-wide vs plain list per collection shape), not an accidental
  fallback. Verify against a real podcast-collection library.
- Podcast wide pills reduce the show-list height by the pill+spacer rows — confirm
  the list scroll/hit map still lines up (it already does in narrow and in Books).

## Migration

Order: land `unify-selected-row-background` first. This change's podcast/book/music
selection rows then already call the shared primitive; do not re-add per-surface
selection paints here.
