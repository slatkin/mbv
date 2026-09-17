# Proposal: home-rows-playback-palette

## Why

Home's rows render through the pre-`now-playing-media-type-titles` title model: a plain
`display_name_parts()` split that only covers Emby episodes and ABS show episodes, painted in the
legacy soft-white/yellow two-tone. Feed rows and audio-track rows carry an unattached title with no
container context, and the split rows do not share the now-playing strip's typed palette, so the
same "container + item title" fact renders with two different colour vocabularies in one app.

## What Changes

- Home's row projection swaps `display_name_parts()` for core's existing
  `QueueItem::playback_title_parts()` mapping, so rows gain the PR's field displays:
  episode → series context, audio track → artist context, feed entry → subscription-name context,
  ABS podcast episode → show context; movies, books, and unresolvable entries degrade to the title
  alone.
- The shell resolves feed subscription display names from configuration at
  `assign_home_content` time (components never see `Config`); `HomeContent` carries a
  feed-id → display-name lookup the projection reads.
- The canonical wide-list painter's two-tone branch adopts the now-playing palette:
  container/context in `PLAYBACK_CONTEXT_FG` (gold `#dbbc7f`), item title in `PLAYBACK_TITLE_FG`
  (aqua `#35a77c`), replacing the yellow focus-accent secondary. Painter-level change, no new row
  types or constructor churn; Home is today's only two-tone producer, so the pixel blast radius is
  Home only.
- Title-only rows keep the ordinary soft-white emphasis — the gold/aqua pair is the *split-row*
  treatment, not a blanket Home recolour.
- Paint order and truncation priority are unchanged (context/container first; the item title keeps
  its width, the container ellipsises first).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `canonical-media-lists`: the two-tone row colour contract changes — the optional secondary title
  paints in the playback-context gold role while the primary (item title) paints in the
  playback-title aqua role on split rows; single-part rows keep the ordinary title role; played
  rows mute the title part while the context part keeps its role.
- `home-latest-sections`: Home rows SHALL render the container context part per media type using
  the shared playback title-parts mapping, with feed subscription names resolved by the shell from
  configuration.

## Impact

- `src/app/render/components/media_list/row.rs` — two-tone colour sites (marquee `parts` vec and
  truncation branch).
- `src/app/components/home_content.rs` — `project_active_section` swaps the parts source.
- `src/app/shell_home_content.rs`, `src/app/types_playback.rs` — feed subscription name resolution
  and `HomeContent` lookup field.
- Tests: `media_list.rs` two-tone pins (`episode_row_paints_secondary_title_in_the_focus_accent_role`
  and marquee/truncation pins) update to the new roles; new coverage for artist/subscription context
  rows and title-only-stays-white.
- No core (`mbv-core`) changes — the media-type mapping and `PlaybackTitleParts` already exist.
