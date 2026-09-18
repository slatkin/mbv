## Why

The podcast episode rows reveal their episode title only on the selected row, so the
resting list needs another column that tells two episodes of one show apart: the publish
date. The two split-row title parts also read too loud for a browse list — a gold context
name next to a sage item title — when the podcast name is the row's resting identity.

## What Changes

- Episode rows carry their publish date in a fixed six-column right-aligned gutter
  (`17 Sep`), painted in a yellow date role of its own. A row without a date reserves no
  gutter.
- The row metadata vocabulary gains a publish-date variant beside the release year; each
  variant carries its own placement and role, so a destination never chooses either.
- The split-row palette moves: the context (container/podcast) part paints the soft-white
  emphasis role instead of the playback-context gold, and the item title paints a light
  grey instead of the split-row sage. The playback strip's own context and title roles are
  untouched, and the treatment stays identical on every list that projects a split row.
- The podcast episode list is the only split-row producer whose context part is a
  container name today, so this is where the change is visible; Home's episode rows carry
  the same shared palette.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `canonical-media-lists`: the row metadata slot's closed vocabulary gains the publish
  date with its fixed right-aligned gutter and role, and the split-row palette's two roles
  change value.
- `audiobookshelf-podcast-browsing`: downloaded episode rows carry their publish date in
  the right-aligned date gutter.

## Impact

- `src/app/components/media_list/mod.rs` — the metadata variant.
- `src/app/render/components/media_list/row.rs` — the gutter's reservation and paint, the
  split-row roles.
- `src/app/render/theme/` + `src/app/palette.rs` — the split-row context role, the light
  grey split-row title value, the date role.
- `src/app/components/podcast_content.rs` — the episode row's date.
- `src/app/ui_util.rs` — the gutter's short date format.
- Queue, TV, music, books, feeds, and inline search are unchanged: they carry no date and
  project no split row.
