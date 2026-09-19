# Proposal: progress-track-uses-grey4

## Why

The unplayed seek/progress track was originally resolved through the `Steel`
palette value (`#46545f`), while the bundled theme correction intended the
neutral `Grey4` value (`#6c6c6c`). Main's commit `1833c13a` subsequently folded
`Grey4` into the palette's mid grey (`#9e9e9e`) and retired `Steel`, so the
landed role now resolves to `Palette::Grey2`.

## What Changes

Record the already-landed theme correction from commit `da73732b` and its
post-merge resolution:

- keep `PROGRESS_TRACK` on the neutral progress-track role, which resolves to
  `Palette::Grey2` (the palette's mid grey, `#9e9e9e`) after main's compaction;
- use main's compact palette and synchronized `docs/palette.json`, where
  `Grey4` (`#6c6c6c`) was folded into the mid grey and `Steel` was retired; and
- cover the unplayed seekbar with a render-buffer test asserting the
  `PROGRESS_TRACK` role itself.

No new capability or UI-design-language requirement is introduced. The change
continues to satisfy the existing semantic-role and private-primitive
requirements, so this record intentionally uses `skip_specs: true`.

## Impact

- `src/app/render/theme/palette.rs` (main's compact palette, with `Steel` retired)
- `src/app/render/theme/mod.rs`
- `src/app/render/components/chrome_player.rs`
- `docs/palette.json` (main's compact palette)
