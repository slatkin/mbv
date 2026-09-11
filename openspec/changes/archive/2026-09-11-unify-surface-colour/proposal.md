## Why

Colour decisions in this app are not single-source, and nobody could tell that from the theme module.
`focus-aware-column-surface` was planned on the assurance that the colours were "change once, change
everywhere". The assurance was true for role *definitions* and false for colour *decisions*, and the
abandoned change proved it: retargeting one role moved the panels that named it, silently repainted
one surface that named it for an unrelated reason (the playback strip, `#3c4841` → `#48584e`), left
three "focused" panels untouched (queue panel, TV episode box, Music track box, which used a second
role with a different value), and never touched a fourth group at all because those screens picked
their own colour bit (`episode_focused`, `track_active`, `chapter_focused`).

The audit at the abandoned change's start commit (`dad91c0f`), re-measured at `23ddf05a` (see
`design.md` § Audit reconciliation), production code only:

- **one visual decision, two roles**: `SURFACE_FOCUSED` `#3c4841` (14 files via the shared lever)
  and `SURFACE_ACCENT_SOFT` `#48584e` (3 sites, named directly, zero lever call sites);
- **two roles, one value**: `SURFACE_RESTING` = `SURFACE_PLAYBACK` = `#333c43`;
- **one role, nine meanings**: `SURFACE_BACKDROP` paints the library column, the wide split gap, the
  wide Music browser panel, a hero content box, the unfocused queue panel, the narrow player's
  recess row, the player's status pill, the pill-bar spacer row, and (via `list_selected_row_bg`)
  every selected-row punch-through — and also aliases `SURFACE_ARTWORK_PLACEHOLDER`;
- **a surface role aliasing a text role**: `SURFACE_FOCUSED` = `TEXT_ACCENT_MUTED`;
- **73 direct role-name production lines in 23 files** against 14 files using the shared lever, so a
  third of the decisions are reachable by rename but not by intent;
- **per-screen colour choices** on top of the roles (the granular focus bits).

So "change the focused panel look" is not one edit, and "the theme is the single source of truth"
was a statement about the file, not about the picture.

## What Changes

Make the *mapping* single-source, not just the definitions:

- **One focus state.** The shell computes one `FocusState` per frame — which column is active and
  which sub-surface inside it holds the cursor — and screens receive it as a fact. Screens stop
  deriving their own colour bits from `episode_focused` / `track_active` / `chapter_focused`.
- **One surface table.** The theme gains a closed `Surface` identity per rendered region and one
  function resolving `(Surface, &FocusState)` to fill and border. Role constants stay as the values;
  the mapping lives in one row per surface, so a level's appearance is changed in one place that
  provably reaches every surface of that level.
- **Screens name surfaces, never roles.** No `SURFACE_*` names and no resolver choices in production
  screens; they pass their surface identity and the focus state the shell gave them.
- **Two guardrails that fail the build.** An ast-grep rule banning surface role names and
  `resolve_surface_*` calls in production screens outside the theme, and a conformance test that
  enumerates surface × breakpoint × focus state and asserts the rendered rect equals the table's
  value — so a missing or wrong painter fails a test instead of reaching the screen.
- **An inventory.** One classification per production paint site (surface identity, nesting level,
  owner painter, rect source, focus dependency) recorded in the change's design and kept honest by
  the guardrail rule and the conformance test.

Kept from `focus-aware-column-surface`, the one behaviour worth keeping: the row above the pill bar
belongs to the library column. The playback panel owns only its own content rows; that row is the
library column's first content row and follows the library column's focus. On today's code that row
is painted by the panel as a recess, so the behaviour needs the right column's focus bit to exist.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `ui-design-language`: the capability must require one surface table as the only source of a
  surface's colour, must forbid a screen from choosing a role or a resolver arm, and must state that
  selection state inside a pane never changes a surface's appearance.

## Impact

- `src/app/render/theme/` — `Surface` identity, the `(Surface, FocusState)` table, focus-state type.
- `src/app/render/components/**`, `src/app/components/**`, `src/app/shell_draw.rs`,
  `src/app/shell_playback.rs` — every production paint site migrates to the table.
- `src/app/layout.rs`, `src/app/render/arrangements/chrome.rs` — `FocusState` replaces the ad-hoc
  `queue_focused` / `right_focused` bits, and gains the row-ownership fix above.
- Guardrails: one ast-grep rule, one conformance test module enumerating the table.
- Tests: buffer expectations move from role names to surface identities plus focus state.
