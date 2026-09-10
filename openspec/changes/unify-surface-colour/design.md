## Context

`proposal.md` carries the audit. What the approach has to fit:

- **The theme is already the only place colours are *defined*.** Raw primitives are private to
  `render/theme/primitives.rs`, roles are public there, and every production site names a role or a
  resolver. That is why the "change once" assurance looked true.
- **What is scattered is the *mapping* from a rendered region to a colour.** Six mechanisms decide it
  today: the shared resolvers (`resolve_surface_focus`, and on the abandoned branch the column/inset
  ones), direct role names (30 production sites), duplicate roles carrying one level's value
  (`SURFACE_ACCENT_SOFT`, `SURFACE_PLAYBACK`), one role carrying six meanings (`SURFACE_BACKDROP`),
  per-screen colour bits (`episode_focused` / `track_active` / `chapter_focused`), and geometry
  owners the screens do not control (the shell's column backdrops, the playback strip).
- **Value aliasing is what makes the scattered mapping undetectable.** `#3c4841` is a focused column
  surface and the strip's fill; `#48584e` is the focused panel and a retired duplicate role;
  `#333c43` is the panel resting colour, the now-playing strip's resting half and the
  `SURFACE_RESTING` role. A rename or retarget therefore moves some surfaces, skips others, and
  leaves every test green, because tests assert roles and a role's value moves with its definition.
- **The picture is the acceptance surface.** Both defects the user found by eye this week
  (the strip's focused colour, the sub-panels that never lit up) were invisible to the test suite.

## Goals / Non-Goals

**Goals:**

- One edit changes one level's appearance everywhere that level is painted, and nowhere else.
- No production screen chooses a colour, a role, or a resolver arm.
- The picture is pinned by tests that name surfaces, not roles: a wrong or missing painter fails.

**Non-Goals:**

- Repainting anything: every surface keeps the colour it has today unless a task says otherwise.
- Replacing the role constants: they stay the values. This change is about the mapping.
- Layout, breakpoints, painter ownership, mouse geometry, or per-screen content.

## Decisions

### D1 — One focus state, computed once

`FocusState { column: Column { Left, Right }, sub: Option<SubSurface> }` is computed once per frame in
the shell (from `PanelFocus`, the visible panel mode, and the active workspace's cursor) and handed to
screens as a fact. `FrameChromeGeometry` stops carrying ad-hoc bits and carries the value; a screen
receives it with its render context.

Today each screen re-derives its own colour bit from its own sub-mode; that derivation is what made
"change the focused panel look" screen-by-screen work.

_Alternative:_ keep per-screen bits and centralise only the colours. Rejected — the bit is the
decision; centralising the colour alone is what the abandoned change did.

### D2 — One surface table: `(Surface, &FocusState) -> SurfaceColors`

The theme gains a closed `Surface` identity per rendered region — column surfaces, panels, inset
boxes, the dialog frame, the recess rows — and one function mapping it plus the focus state to a
`SurfaceColors { fill, border }`. One row per surface, each row carrying its nesting level. Roles
stay as the values, so a level's appearance is still changed by editing one role, and the table
proves which surfaces that reaches.

The levels are, shallowest first: **column/pane** — the surface a panel sits on, including the column
gutters the shell paints and the hero pane fill; **panel** — a focusable panel body (queue panel,
library panel, a screen's episode/track/chapter box, a hero content box, a dialog's list); **recess**
— a non-focusable inset inside a panel (the now-playing panel's own rows and status pills, the
visualizer background); **dialog** — an overlay frame that never follows a panel. A surface's level
is what an edit to that level reaches, so two surfaces at the same level share their appearance and
shall not be given separate ones.

_Alternative:_ a registry keyed by strings/ids so screens can add surfaces. Rejected — the closed
enum is what makes "every surface is in the table" checkable.

### D3 — Screens name surfaces, never roles or resolver arms

A production paint site calls the table with its `Surface` and the focus state it was given. It does
not name `SURFACE_*`, does not call a resolver, and does not branch on a focus bit to choose a
colour. Branches on focus stay only for behaviour (selection gating, cursor, hit geometry).

### D4 — Two guardrails, both failing the build

- An ast-grep rule fails on surface role names or `resolve_surface_*` calls in production screens
  outside `src/app/render/theme/` (tests excluded).
- A conformance test enumerates every `Surface` × breakpoint × focus state from the same table the
  production code uses and asserts the rendered rect's fill equals the table's value. The table is
  therefore the inventory: a surface with no painter, or a painter the table does not know, fails.

_Alternative:_ a written inventory only. Rejected — a document cannot fail a build, and the abandoned
change's task-1.1 classification (a grep-derived table) is exactly how the four defects were missed.

### D5 — The row above the pill bar belongs to the library column (the one behaviour kept)

`chrome_geometry.player_area` is four rows; only the first three are the playback panel's content.
The fourth is the library column's first content row and must render the library column surface, not
the panel's recess. `QueueOnly` keeps the row as the panel's own (its "On Now" title lives there);
the mini-view discriminator is `narrow_player`, i.e. `effective_panel_mode() == PanelMode::QueueOnly`.
This is the only behaviour salvaged from `focus-aware-column-surface`.

The ownership is **geographic, not incidental**: the row is the library column's first content row,
not "the row the panel happens to leave free". A layout that no longer paints the playback panel in
the wide layouts therefore changes nothing about this row — it is already the column's — and the
panel's own three content rows above it are that layout's business, not this change's. The buffer
test asserts the row's own fill, so the invariant survives the panel being removed.

_Note for the migration in section 4_: the row's focused arm resolves to today's `SURFACE_FOCUSED`
while the resting arm is `SURFACE_BACKDROP` (`#2d353b`), deliberately **not** the panel resting role
(`SURFACE_RESTING`, `#333c43`) that the neighbouring queue arm uses. Substituting the panel resolver
here would silently repaint the resting column; the table's column entry is the fix, and until then
the site names its decision explicitly.

_Accepted consequence (reviewed, section 1)_: the backdrop rect spans the whole right column, so this
also makes the column's entire gutter follow focus, not only the row above the pill bar — in wide
`LibraryOnly`, where focus is always the library, the column gutter is permanently `#3c4841`. That is
the column-surface behaviour itself and was accepted as a wider visible delta than the row alone; the
buffer tests pin it at wide `LibraryOnly`, wide `Both` library-focused and wide `Both` queue-focused.

## Risks / Trade-offs

- **[A big mechanical migration can drift the picture]** → every migrated site's expected value is
  today's value; the conformance test pins the picture surface-by-surface, and the change lands in
  screen-sized units so any drift is one screen wide and reviewable.
- **[The closed `Surface` enum can lag a new region]** → the ast-grep rule fails on any role name in a
  screen, so a new region cannot choose a colour at all; it must add a `Surface` row.
- **[Value aliasing can survive inside the table]** → two surfaces may still share a value; that is
  fine (a value is not a level). The table states each surface's level, so a level's change is one row
  group, and the audit of shared role values is a row in the tasks.

## Migration Plan

Sections in `tasks.md` land in order, each as its own reviewable unit: the kept behaviour, the
inventory, the focus state, the table plus per-screen migration, the guardrails, then the gates. No
persistence, protocol, or config. Rollback is a revert.

## Open Questions

- Whether `FocusState` should also carry the mouse-hover surface once ADR 0024's hit ownership is
  revisited. Deferrable: hover is not a colour input today.
