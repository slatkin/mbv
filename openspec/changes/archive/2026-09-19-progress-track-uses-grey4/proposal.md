# Proposal: progress-track-uses-grey4

## Why

The unplayed seek/progress track was resolved through the `Steel` palette value
(`#46545f`), while the intended neutral progress-track colour is `Grey4`
(`#6c6c6c`). The role and its buffer coverage should agree with the approved
palette semantics.

## What Changes

Record the already-landed theme correction from commit `da73732b`:

- resolve `PROGRESS_TRACK` to `Palette::Grey4`;
- retain `Steel` as an approved but currently unused palette variant, marked
  dead-code-expected for future semantic use;
- update `docs/palette.json`; and
- cover the unplayed seekbar with a render-buffer test asserting the Grey4
  value.

No new capability or UI-design-language requirement is introduced. The change
continues to satisfy the existing semantic-role and private-primitive
requirements, so this record intentionally uses `skip_specs: true`.

## Impact

- `src/app/render/theme/palette.rs` (marking `Steel` as approved unused)
- `src/app/render/theme/mod.rs`
- `src/app/render/components/chrome_player.rs`
- `docs/palette.json`
