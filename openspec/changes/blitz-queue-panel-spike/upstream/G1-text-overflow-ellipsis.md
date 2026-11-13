# Upstream draft G1: `text-overflow: ellipsis` is not painted

Status: draft. Not posted. The user approves each draft before anyone posts it (decision 6).

Gap: design G1 in `design.md`.

## Target

Do not open a new issue. Issue #888 already covers this gap and is open. Post a comment on https://github.com/DioxusLabs/blitz/issues/888 if the user approves. Two open pull requests implement the same feature, so the comment should point to them, not repeat them:

- PR #893 (`Implement text-overflow`, open). It paints the marker in `blitz-dom` and `blitz-paint`.
- PR #1015 (`Implement text-overflow (continues #893)`, open). It rebuilds the cut in layout and adds right-to-left blocks.

## What was searched (2026-11-13)

Read-only searches with `gh`. Nothing was posted.

- Blitz issues, all states, for `text-overflow`, `ellipsis`, and `truncate`. Matches: #888 (open), #841 (scroll and transform, not this gap), #363 (browser UI, not this gap).
- Blitz pull requests for `text-overflow`. Matches: #893 and #1015 (both open). Their titles and bodies were read.
- Blitz source at pinned `blitz-dom` and `blitz-paint` 0.3.0-beta.4 (`~/.cargo/registry`): `rg -i 'text.overflow'` finds no marker handling. Only `overflow_wrap` is handled (`stylo_to_parley.rs`).
- WPT (`web-platform-tests/wpt`), CSS Overflow directory, names that match `text-overflow`. Tests found and read or named:
  - `css/css-overflow/text-overflow-ellipsis-001.html`, with the reference `reference/text-overflow-ellipsis-001-ref.html` (the basic ellipsis placement; read);
  - `css/css-overflow/text-overflow-001.html` (exists; not read in full);
  - `css/css-overflow/text-overflow-string-001.html` (the string form; exists; not read in full);
  - `css/css-overflow/text-overflow-ellipsis-rtl-001.html` (exists; not read in full).

## Summary for the comment

`text-overflow: ellipsis` is parsed by Stylo. In `blitz-dom` 0.3.0-beta.4 and on `main`, no code path paints the marker. An overflowing line in a clipping inline root is cut and shows no marker. The WPT test `text-overflow-ellipsis-001.html` checks the placement of the marker. It would show the gap as a mismatch with its reference.

## Minimal repro

Expected: a line of text is cut at the right edge of the 200 px box, and an ellipsis (`…`) is painted at the cut.

```html
<!DOCTYPE html>
<meta charset="utf-8">
<style>
  .row {
    width: 200px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font: 16px/1.2 sans-serif;
  }
</style>
<div class="row">A very long queue row title that does not fit in the box</div>
```

Actual in 0.3.0-beta.4: the text is clipped at the edge. No marker is painted.

Status of this repro: not run. The expected and actual results come from the source read and from the description in #888. Run the repro on a Blitz build before posting.

## Notes for the user

- The spike's queue rows use `truncate` (`overflow: hidden`, `text-overflow: ellipsis`, `white-space: nowrap`). Until PR #893 or #1015 merges, the spike shows the rows clipped with no marker. That is the expected G1 result in the design.
- Do not post this draft until the user approves it. The comment must not reproduce PR text.
