# Tasks

## 1. Record the landed correction

- [x] 1.1 Resolve `PROGRESS_TRACK` to the palette's mid grey,
  `Palette::Grey2` (`#9e9e9e`), after main's `1833c13a` compaction folded
  `Grey4` (`#6c6c6c`) into that variant and retired `Steel` (`#46545f`).
- [x] 1.2 Adopt main's compact palette and synchronized `docs/palette.json`,
  with `Grey4` folded into the mid grey and `Steel` retired.
- [x] 1.3 Add the `chrome_player` render-buffer test proving an unplayed
  seekbar uses the `PROGRESS_TRACK` role itself.

## 2. Verification

- [x] 2.1 Archive this change record with the already-landed implementation.
- [x] 2.2 No delta spec is needed: the correction remains within the existing
  `ui-design-language` requirements; validate with `skip_specs: true`.
