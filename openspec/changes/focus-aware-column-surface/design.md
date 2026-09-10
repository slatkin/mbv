## Context

See `proposal.md` for the motivation. The facts below are what the approach has to fit.

### Three greens already behave as a nesting model — in one column only

`src/app/render/theme/primitives.rs` holds the raw values; `theme/mod.rs` is the public role API.

```
                 focused     unfocused   role today                 who resolves it
column/backdrop  #3c4841     #2d353b     SURFACE_FOCUSED / BACKDROP  only the QUEUE column does this
panel            #48584e     #333c43     SURFACE_ACCENT_SOFT         queue panel, TV episode box,
                                        / SURFACE_RESTING           Music track box (3 sites)
```

The queue column (`chrome.rs:33` left arm, via `resolve_surface_focus(queue_focused)`) and its panel
(`widgets.rs:237`) already form the pair the proposal describes. The library column does not: the
right arm of the same function paints `SURFACE_BACKDROP` unconditionally.

### The panel lever is already the single control point

`theme::resolve_surface_focus(bool)` has 16 production call sites — every library rail, Home, Feeds,
TV, Music, Audiobookshelf, the queue boundary, the card, the Wide hero pane, and the selected-row
surface. Retargeting its focused arm moves all of them with no screen edit. The 11 sites that name
`SURFACE_FOCUSED` directly do **not** follow it: 10 are inside the nine modal frames (each of the
nine `render_modal_frame` callers plus one body paint in `selection_modal.rs`) and 1 is the
now-playing strip (`shell_playback.rs:48`).

### `SURFACE_BACKDROP` is one value standing in for four different things

| site | means |
| --- | --- |
| `chrome.rs:39`, `home.rs:397`, `music_wide.rs:543` | the library/music column surface |
| `list_selected_row_bg()` (6 call sites, 3 files) | the selected-row punch-through |
| `wide_hero.rs:414` | the recessed content box inside a hero pane |
| `chrome_player.rs:122/158/239` | the recess inside the now-playing panel |
| `widgets.rs:239` | the *unfocused* queue panel |

Only the first two need to move. The rest are genuinely "a recess inside a surface" and keep the
value.

### Two facts that keep the diff small

- A canonical list paints its selected-row background only while it is focused
  (`wide_row.rs`: `let selected = selected && focused`), so the unfocused value of the row colour is
  never rendered on that path. The legacy `item_cell_spans`/`build_list_row_spans` paths do not gate
  on focus, so their row colour must be focus-resolved to stay byte-identical when unfocused.
- The Wide hero pane already renders the target values in all three of its cases: `Workspace(true)`
  → `#3c4841`, `Workspace(false)` and `ReadOnly` → `#333c43`.

### Where a screen learns the right column's focus

`render_legacy_backdrops` (`chrome.rs:14`) paints both columns, and it is called from
`paint_legacy_chrome` with a `FrameChromeGeometry` that already carries `queue_focused`, derived in
`arrangements/chrome.rs` from the `PanelFocus` the shell supplies. The mirrored bit does not exist
yet, but its input does.

## Goals / Non-Goals

**Goals:**

- One nesting model — column/pane surface, panel, recess, dialog overlay — expressed as roles.
- The library column's surface follows panel focus, so the focused column is apparent without
  reading the panel fill.
- The selected-row highlight derives from the surface containing its panel, on every list renderer.
- No per-screen colour decision is introduced, and a future change to any one level is still a
  one-line edit.

**Non-Goals:**

- Changing the unfocused appearance of anything. Every unfocused arm keeps its current value.
- The unfocused queue panel (`widgets.rs:239`, `#2d353b`) currently borrows the column's resting
  colour, so an unfocused queue reads as a flat column. That is a real role misuse but a separate
  visual decision; it is not fixed here.
- Recolouring dialog frames, the tab bar, the status bar, the player panel's recess rows, or the
  recessed hero content box.
- Any change to layout, breakpoints, painting ownership, or mouse geometry.

## Decisions

### D1 — Four roles, one per nesting level

`SURFACE_COLUMN_*` (column/pane surface), `SURFACE_PANEL_*` (panel), `SURFACE_BACKDROP` (recess),
`SURFACE_DIALOG` (overlay frame). Both column and panel get focus-resolved resolvers.

_Alternative:_ keep `SURFACE_BACKDROP` polymorphic and only make the three column sites
focus-aware. Rejected — that is the current state: one constant, four meanings, three screens
resolving it by hand. The spec requirement this change adds ("one role per nesting level") is what
prevents the next screen from re-deriving an answer.

### D2 — Retarget the existing panel lever; add a second one

`resolve_surface_focus(focused)` keeps its name and shape and returns `#48584e` / `#333c43`.
A new `resolve_surface_column(focused)` returns `#3c4841` / `#2d353b`. Call sites of the lever are
untouched.

_Alternative:_ introduce `SURFACE_PANEL_FOCUSED` and rename `resolve_surface_focus`. Rejected —
16 call sites and 42 test assertions for no behavioural gain.

### D3 — Retire `SURFACE_ACCENT_SOFT` into the panel role

Its three production sites (queue panel, TV episode box, Music track box) are all "a focused panel",
which is exactly what the lever's focused arm now returns. Two roles for one level is what the
`ui-design-language` spec forbids.

_Alternative:_ keep it as an alias. Rejected — an alias that must be kept equal to another role by
discipline is the drift this change exists to remove.

### D4 — The Wide hero pane is a pane surface, not a panel

`LeftPaneFocus::Workspace(held)` resolves through the column/pane role; `ReadOnly` stays on the
resting value. This is the one place where the classification matters visually: the pane contains a
recessed content box, so if the pane joined the panel role the box would vanish into its parent, and
the Music track list and TV episode list (whose rows punch through to the pane) would lose their
selection contrast.

_Alternative (rejected):_ pane as panel (`#48584e` when held).

### D5 — Dialog frames get their own role, value unchanged

`SURFACE_DIALOG = #3c4841`. The nine modal frames' 10 direct-name call sites move to it in the same
commit, before the lever is retargeted, so the retarget cannot reach them.

_Alternative:_ let dialogs follow the panel role to `#48584e`. Rejected — an unrequested visual
change to nine overlay surfaces, and it would make a dialog indistinguishable from a focused panel
behind it.

### D6 — `SelectedRowSurface` is deleted

Once the library column follows focus, `ListBackdrop` and `OwningSurface` resolve to the same colour
for every caller — the distinction exists only to encode the column's focus-blindness. Keeping a
two-variant policy whose variants are provably equal is dead flexibility, so the enum, the two paint
policy fields, and their ~20 call-site arguments go; the row resolver takes only the focus bit.

_Alternative:_ keep the enum as documentation. Rejected — it would need a comment explaining that
one of its variants has no effect, which is worse than deleting it.

### D7 — The shell owns the column fill; screens never paint it

`FrameChromeGeometry` gains `right_focused` (from the `PanelFocus` it already receives), and the
right-column backdrop fill resolves from it. Destination screens keep painting only their own panel
and any column-surface inset *inside* their own rect (the Home spacer, the Music browser panel),
each through the same resolver.

_Alternative:_ let each destination paint its column. Rejected — the destinations' rects do not
cover the column (the gutter, the pill row and the spacer are outside the list panel), so it would
mean several owners for one surface.

## Risks / Trade-offs

- **[The column colour has more than one paint site, and they can disagree]** → every column-surface
  site resolves from the same function; add one buffer assertion per breakpoint (LibraryOnly,
  Both-when-library-focused, wide music) that the visible gutter around a focused library panel
  equals `resolve_surface_column(true)`.
- **[A naked `SURFACE_FOCUSED` site is missed and gets repainted]** → the 11 direct-name sites are
  enumerated and classified in the tasks before the lever is retargeted; the classification is
  mechanical (`cargo check` finds nothing, so it is verified by the existing dialog buffer tests
  plus one assertion that `SURFACE_DIALOG` differs from the panel role).
- **[Contrast regression: a row punching through to `#3c4841` inside a `#48584e` panel]** — a
  smaller step than the queue's existing pair, so it is the same step the app already ships; → the
  row keeps its emphasis foregrounds, and the existing focus-colour tests are extended to assert the
  row is distinct from the panel body (already the shape of `tv_wide_tests`' assertion).
- **[Unfocused appearance drifts]** → every unfocused arm keeps its current value by construction
  (`#333c43` for panels and panes, `#2d353b` for columns), and the legacy row paths are verified
  byte-identical when unfocused.

## Migration Plan

One commit, no persistence, no protocol, no config. Rollback is a revert. No surface is migrated
between owners, so no ledger update is needed.

## Open Questions

- Whether the unfocused queue panel should adopt the resting panel colour instead of the column's
  resting colour. Deferrable: it is a visible change to one surface that neither spec nor tasks
  depend on, and both values already exist as roles.
