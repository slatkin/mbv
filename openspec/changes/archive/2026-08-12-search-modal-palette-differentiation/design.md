## Context

The search modal renderer (`src/app/render/overlays/search_modal.rs`) threads `palette::LIBRARY_SIDE_BG` as a hard-coded constant into every call that fills a background: the modal frame, result rows, state messages, and type-filter chip gaps. The modal's `SearchMode` is already available on the `SearchModal` struct, so mode-dependent styling is a matter of selecting the colour at each call site based on `modal.mode`. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**
- Make the modal body background visually different between fuzzy and global mode using two palette colours that already exist.
- Keep the change confined to the render file; no new state, no new palette entries, no protocol or daemon changes.

**Non-Goals:**
- Changing any other modal styling (input row, borders, text, hero rules).
- Adding a user-configurable option for these colours.
- Changing the selected-row or hero background.

## Decisions

**Decision 1: Introduce a small helper rather than repeating a match at every call site.**

Add a function like `fn body_bg(mode: SearchMode) -> Color` in the same render file that returns `LIBRARY_SIDE_BG` for `Global` and `BG_GREEN` for `Fuzzy`. Every background fill in the file calls this helper instead of referencing the constant directly.

Rationale: there are at least five call sites (modal frame, two `render_state_message` calls, result row bg, type-filter gap). A match at each site duplicates the mapping and makes it easy to miss one — which is exactly what happened in the first failed implementation. A single helper concentrates the mapping in one place and makes a missed site a compile-visible omission.

Alternative considered: threading the colour as a parameter from the top-level `render_search_modal` function. This works but pushes a `match` into the caller and makes the helper approach redundant; the helper is the simpler form.

**Decision 2: Do not introduce a new palette constant.**

`BG_GREEN` (#3c4841) and `LIBRARY_SIDE_BG` (#2d353b) already exist in `src/app/palette.rs`. No rename, no alias.

Rationale: adding a semantically named alias (e.g. `SEARCH_MODAL_FUZZY_BG`) creates a second source of truth that has to be kept in sync with the palette comment. The existing palette already has comments with the hex values, and the helper function's name makes the intent clear at the call site.

## Risks / Trade-offs

**[Risk] A call site is missed, reproducing the original no-visible-change failure.** → Mitigation: the helper function means there is exactly one `match` on `SearchMode` for the background. Review the diff with a text search for `LIBRARY_SIDE_BG` in the render file; every remaining occurrence should be either the helper's own definition or a non-body site that legitimately stays (none expected).

**[Risk] The green tint is too subtle to read as a deliberate mode indicator.** → Mitigation: out of scope for this change's code; if the colour is wrong, adjust the palette constant or the mapping in the helper. The spec is satisfied as long as the two modes produce different background values.
