## Context

The app is a ratatui TUI. All widget colors in `src/app/palette.rs` are explicit `Color::Rgb(...)` values — there's no terminal-palette/theme indirection. Modal overlays (`confirm_modal.rs`, `save_playlist_dialog` in `playlists.rs`, `multiselect.rs`, `library_routes.rs`) are drawn late in `App::render` (`src/app/render/mod.rs`), after the main view and any docked panels, by rendering a `Clear` widget over a centered `Rect` and drawing a bordered `Block` inside it. Nothing currently touches the cells outside that `Rect`.

Ratatui has no alpha compositing. The only way to visually "dim" content that's already been drawn into the frame's `Buffer` is to rewrite the color of every cell in the background region before the modal's own widgets are drawn on top. The `Modifier::DIM` terminal attribute exists but its effect on true-color (RGB) foreground/background pairs is terminal-dependent and unreliable — since every color in this app is explicit RGB, it's safer to blend the actual RGB values than to depend on a terminal's own dim rendering.

## Goals / Non-Goals

**Goals:**
- Every centered/blocking modal overlay dims the full terminal area behind it before drawing its own content, so the modal reads as a focused layer above dimmed content — matching opencode's modal treatment.
- One shared helper, so each modal's render function adds a single call rather than reimplementing dimming.
- Deterministic, terminal-independent result (same look in any terminal emulator).

**Non-Goals:**
- Docked panels (sessions, playlists, help, settings) and the small anchored context menu are not in scope — they aren't blocking modals and already coexist with visible app content by design.
- No animation/fade — the dim is applied once, synchronously, as part of the existing render pass.
- No configurability (dim strength, on/off toggle) — not requested.

## Decisions

- **Buffer cell blending over `Modifier::DIM`.** Iterate the cells in `f.buffer_mut()` covering the full frame area (or the region behind the modal) and blend each cell's fg/bg `Color::Rgb` toward black by a fixed factor (e.g. multiply each channel by ~0.5). This is deterministic and matches how the palette already models color (plain RGB, no terminal-attribute reliance). Non-`Rgb` `Color` variants (none currently used in this codebase) are left untouched rather than guessed at.
- **Shared helper on `App`, called once per modal before its existing `Clear`/`Block` drawing.** Add `render_backdrop_dim(&self, f: &mut Frame)` (or similar) in a small new file under `src/app/render/overlays/` (e.g. `backdrop.rs`), covering the full `f.area()`. Each of the four modal render functions calls it first, unchanged otherwise.
- **Dim the whole frame, not just the area outside the modal's `Rect`.** The modal's own `Clear` + `Block` draw over the dimmed cells immediately after, so the modal itself is unaffected. This avoids computing an inverse/cutout region and keeps the helper trivial.
- **Scope to the four true modals only** (confirm modal, save-playlist dialog, multiselect popup, library-routes popup), matching the proposal. Docked panels stay as-is.

## Risks / Trade-offs

- [Double-dimming when modals stack, e.g. a confirm modal opened from within the multiselect popup] → Each modal always dims from the current buffer state, so a second dim pass would darken an already-dimmed background further. Acceptable: stacked modals are rare and a slightly darker backdrop is not visually broken. Not fixing further in this change.
- [Blending by a fixed multiplier could reduce contrast/readability of any background text still faintly visible] → Keep the factor conservative (~0.5) and verify visually against the existing palette; this is a visual-only change with no functional impact if slightly off, easy to tune post-merge.
- [Manual buffer manipulation is more low-level than typical ratatui widget usage] → Contained entirely in one small helper function; the four call sites stay one-line additions.

## Open Questions

None — approach confirmed against the existing render code before writing this design.
