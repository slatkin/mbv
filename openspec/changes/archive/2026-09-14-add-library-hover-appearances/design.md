## Context

See `proposal.md` — Why and `specs/mouse-input/spec.md` for observable behavior.

ADR 0024 already delivers every raw mouse event, including `MouseEventKind::Moved`, to each component that was painted in the latest frame. It requires kind filtering inside the component, point resolution against self-painted retained geometry, and component-local hover state. The delivery, eligibility, overlay arbitration, and single-message fold therefore already have the required shape.

`TabPanel` retains `(Rect, tab_position)` for each visible painted tab. `LibraryPanel` retains separate `HitRegions<usize>` registries for its main Selector row, List controls, and Workspace selector; the main Selector row is painted through `paint_selector_row` and the shared `render_pill_bar`. The shared pill painter is also used by excluded surfaces, so hover cannot become an implicit global `PillBar` behavior.

## Goals / Non-Goals

**Goals:**

- Keep hover ownership and invalidation inside the Interactive Component that owns each painted surface.
- Reuse the latest-frame tab and Selector-row hit geometry already used for clicks.
- Keep the painter's visual state explicit so excluded `PillBar` callers remain unchanged.
- Make the first style cheap to tune after evaluating it in the running TUI.

**Non-Goals:**

- Introduce a general hover framework, global pointer position, global hit map, shell request, or persisted hover state.
- Add timing, animation, delayed hover, tooltips, cursor changes, or hover behavior to embedded media lists.
- Add `HoverEnter` / `HoverLeave` to `MouseGestureState` before a consumer needs semantic hover gestures beyond local paint state.

## Decisions

### D1 — Resolve raw movement directly into component-local target identity

`TabPanel` holds `hovered: Option<usize>` containing a tab position. `LibraryPanel` holds a separate `hovered_selector: Option<usize>` containing only the main Selector-row pill index. On every delivered `MouseEventKind::Moved`, each component resolves the point against its own latest retained hit regions and replaces that value, including with `None` for gaps and other surfaces.

The movement arm returns no `Msg`: changing hover requests no shell-owned work. Existing click and gesture behavior remains unchanged. Re-rendering follows the existing tick/draw path.

Alternative: extend `MouseGestureState` with enter/leave variants. Rejected for this scope because no semantic event crosses a boundary and comparing the newly resolved `Option<usize>` with local state already expresses enter, move, and leave. Add gesture variants only when a future consumer needs timing or semantic hover gestures.

Alternative: store pointer coordinates and ask the shell or painter to resolve them. Rejected because ADR 0024 assigns resolution to the component that painted the geometry and forbids shell re-resolution.

### D2 — Main Selector-row hover is an explicit input to the shared pill painter

The shared `PillBar` paint model accepts an optional hovered pill identity in addition to its selected position. `paint_selector_row` supplies `LibraryPanel`'s local main-selector hover identity. Workspace-selector and selection-modal callers explicitly supply no hovered identity, preserving their current output.

Selected state wins over hover in the painter's closed style policy:

```text
selected --> selected style
else hovered --> hover style
else --> resting style
```

Alternative: infer hover from a globally configured pill style or make every `PillBar` caller hoverable. Rejected because it expands behavior to explicitly excluded surfaces.

### D3 — Start by composing existing semantic theme roles

The initial unselected tab hover uses the existing strong text role without adding a background block. The initial unselected pill hover uses the existing focused fill resolution for the ordinary Pill chip plus stronger text. Selected targets keep their current selected style unchanged.

This is a provisional visual choice, not a new caller-controlled Variant. The implementation includes an explicit manual evaluation checkpoint. If the existing semantic roles do not produce the desired contrast, tuning remains centralized in the tab/pill style policies; add a dedicated semantic hover role only if evaluation shows the existing vocabulary cannot express the desired appearance.

Alternative: add new raw colors immediately. Rejected because the existing semantic vocabulary can produce the first runnable comparison, and raw painter-selected colors would violate the theme boundary.

### D4 — Verify state ownership, painting, and live delivery separately

Focused component tests establish that movement over a retained target updates only local hover state, movement into a gap clears it, and no `Msg` is returned. Buffer tests establish the resting/hovered/selected precedence and prove excluded pill callers remain unchanged. A live `Application::tick()` integration test establishes that a movement event reaches the subscribed Tab and Library panels through the real synchronization pass and changes only the surface under the point.

Both Narrow and Wide Library panel presentations are covered because the main Selector row is painted in both with different placement geometry. Tests assert rendered style/content and containment in retained role rectangles rather than fixed coordinates.

## Risks / Trade-offs

- **[Terminal or multiplexer does not report passive movement]** -> Click behavior remains intact; hover is optional feedback and no action depends on it.
- **[Pointer leaves the terminal without a final movement event]** -> The last hover may remain until the next terminal mouse event; do not add polling or a global cursor tracker for cosmetic state.
- **[High movement rate causes unnecessary work]** -> Update local state only when the resolved target identity changes; rely on the existing event and draw loop unless profiling demonstrates a problem.
- **[Selected and hovered appearances become ambiguous]** -> Keep selected styling dominant and require manual evaluation before acceptance.
- **[Shared `PillBar` accidentally changes excluded callers]** -> Make hover explicit and optional in its paint model and retain unchanged-output buffer coverage for excluded uses.

## Migration Plan

No data or configuration migration is required. The change is additive and can be rolled back by removing the two local hover fields and optional paint inputs; click behavior and retained geometry remain intact.
