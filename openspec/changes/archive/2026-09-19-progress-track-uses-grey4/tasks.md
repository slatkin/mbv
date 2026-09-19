# Tasks

## 1. Record the landed correction

- [x] 1.1 Resolve `PROGRESS_TRACK` to `Palette::Grey4` (`#6c6c6c`) instead
  of `Steel` (`#46545f`).
- [x] 1.2 Mark the now-unused `Steel` palette variant as approved dead code
  and synchronize `docs/palette.json`.
- [x] 1.3 Add the `chrome_player` render-buffer test proving an unplayed
  seekbar uses the progress-track role's Grey4 value.

## 2. Verification

- [x] 2.1 Archive this change record with the already-landed implementation.
- [x] 2.2 No delta spec is needed: the correction remains within the existing
  `ui-design-language` requirements; validate with `skip_specs: true`.
