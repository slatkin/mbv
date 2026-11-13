# Upstream draft G8: failed fetches do not reach the image and font handlers

Status: draft. Not posted. The user approves each draft before anyone posts it (decision 6).

Gap: design G8 in `design.md`.

## Target

Do not open a new issue. An open pull request covers the image path, and a closed issue covers the same contract:

- PR #1423, `Render broken images as their alt text` (open). It makes `ImageHandler` forward `NetHandler::error`. It also adds a WPT runner change that calls `handler.error` for a missing file.
- Issue #694, `Failed network fetches never call back, leaving critical resources pending forever` (closed 2026-11-02, completed). The fix was PR #1318 (merged 2026-10-28).

Post a comment on https://github.com/DioxusLabs/blitz/pull/1423 if the user approves. The comment asks that the same forwarding apply to `FontFaceHandler`. The no-`net` fallback in dioxus-native is a separate report (see G9). Do not open a duplicate issue.

## What was searched (2026-11-13)

Read-only searches with `gh` and the cargo registry. Nothing was posted.

- Blitz issues, all states, for `error NetHandler`, `net provider error`, `image load error`, `NetHandler`, and `resource error`. Matches: #694 (closed, see above), #1419 (withdrawn, not this gap), #896 (renderer-owned textures, a different gap, G7).
- Blitz pull requests for `net provider`, `custom net provider`, and `NetHandler`. Matches: #1423 (open), #1318 (merged, via #694), #1374 (merged 2026-11-03, the `resolve_url` panic path, not this gap).
- Dioxus issues and pull requests for `NetProvider` and `DataUriNetProvider`: none on this gap (see G9).
- Source at pinned `blitz-traits` and `blitz-dom` 0.3.0-beta.4 (cargo registry): `NetHandler` has only `bytes()`. `blitz-dom/src/net.rs` has no `fn error`.
- Source on `main` (`DioxusLabs/blitz` commit `563d3db73`, read through `gh api`): `NetHandler` has `error()` with a no-op default. `ResourceHandler<StylesheetHandler>` forwards it (line 146). `ResourceHandler<ImageHandler>` (line 595) and `ResourceHandler<FontFaceHandler>` (line 302) implement only `bytes()`.
- WPT (`web-platform-tests/wpt`), tests that cover broken images, names read or listed:
  - `html/semantics/embedded-content/the-img-element/img-complete-after-error.html` (exists);
  - `html/semantics/embedded-content/the-img-element/404-response-with-actual-image-data.html` (exists);
  - `html/rendering/replaced-elements/attributes-for-embedded-content-and-images/img-no-alt-replaced.html` and `img-empty-alt-replaced.html` (exist; they cover `alt` on a replaced image, not a broken one);
  - No WPT test for the alt text of a broken image was found in `html/rendering/replaced-elements/images` or `attributes-for-embedded-content-and-images`.

## Summary for the comment

A failed image fetch has no path to the image handler on `main`. `ImageHandler` and `FontFaceHandler` do not implement `error()`, so the default no-op runs. The error result is lost, and the node has no broken state. #1423 fixes the image case. The font case has the same gap. In `blitz-dom` 0.3.0-beta.4, `NetHandler` has no `error()` method at all, so every failure path drops the handler.

## Minimal repro

Expected (browser): the 160 × 90 box shows the broken-image state with the alt text `Cover art`.

```html
<!DOCTYPE html>
<meta charset="utf-8">
<img src="https://example.invalid/missing.png" alt="Cover art" width="160" height="90">
<p>Text after the image.</p>
```

Actual in 0.3.0-beta.4 and on `main` before #1423: the image box is empty, and the alt text is not painted. The paragraph is laid out below the box. The fetch failure is not visible to the document.

Status of this repro: not run. The expected and actual results come from the source read, and from the description in #1423 (`renders as nothing`). Run the repro on a Blitz build before posting.

## Notes for the user

- The spike does not depend on the error callback. The `MbvNetProvider` answers a missing art path with the placeholder bytes (design D6), so the image is never a failed fetch in the spike. G8 stays in the gap table as `absent`.
- The design's G8 evidence says "`NetHandler::error` defaults to a no-op". That is true on `main` only. In 0.3.0-beta.4 the method does not exist. The design text was corrected to match.
- Do not post this draft until the user approves it.
