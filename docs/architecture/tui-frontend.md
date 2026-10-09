# TUI frontend guide

Read before any TUI change: a component, painter, arrangement, theme role, key,
mouse event, component-local state, or visual variant — including one-line
tweaks. Term definitions live in `CONTEXT.md` under Presentation and win any
disagreement with this file.

Covered elsewhere, not repeated here:

* Component/shell boundary and one-way projection — `AGENTS.md` "Interactive
  architecture", ADR 0022.
* Keyboard routing (one site: `src/app/input/router.rs` + `key_policy.rs`) —
  ADR 0023, including why a second resolution site or `SubClause` precedence
  fails.
* Mouse delivery — ADR 0024, `openspec/specs/mouse-input/spec.md`.
* Embedded canonical media lists — `openspec/specs/canonical-media-lists/spec.md`.
* Current surface ownership — `docs/architecture/interactive-surface-ledger.md`.

## Two trees, not one

The trees have colliding file names: `crates/mbv-components/src/help.rs` and
`crates/mbv-render/src/components/help.rs` are different things.

| Tree | What lives there | Example |
|---|---|---|
| `crates/mbv-components/src/` | **Interactive Components** — TuiRealm `impl Component`: local interaction state, event interpretation, `update()`, and the `view()` entry point | `HelpComponent` |
| `crates/mbv-render/src/` | **The visual substrate** — painters, arrangements, and layout a component's `view()` calls into | `render_help_panel` |

`Component::view(&mut self, f, area)` receives an outer area from an arrangement
or the root layout and delegates painting to `mbv-render`. A component holds
cursor, scroll, drafts, viewport, and hit geometry; `mbv-render` holds pixels.
When unsure, ask "is this interaction, state, painting, layout, or style?" and
route to `mbv-components` / `screens/` / `components/` / `arrangements/` /
`crates/mbv-theme/`.

`crates/mbv-render/src/` modules (from the archived
`enforce-mbv-ui-design-system` change):

| Module | Owns | Must not |
|---|---|---|
| `screens/` | app state in, typed content model out | call Ratatui, construct a `Rect`, compute a hit target |
| `arrangements/` | placement of components within a `Rect`, breakpoints | own painting or app state |
| `components/` | painting, its own geometry within a `Rect` | take arbitrary `Color`/`Style` from a screen |
| `crates/mbv-theme/` | roles and surface rows resolved through the active theme (public) | expose slots, the theme value, or colour literals (private) |

Dependency order: `screens -> arrangements -> components -> Ratatui`.
Rendering never performs Service/image/playback/persistence effects.

## Panel and slot composition

The root composes the Tab, Library, Library playback, Queue, Queue playback,
Status bar, boundary, and overlay Panels. A Panel owns placement, fills, slots,
painting, and retained hit geometry; no base frame paints beneath it. A
destination supplies typed slot content only. If a needed element has no slot,
extend the Panel content type for every destination rather than adding a
destination painter or caller-selected arm.

The Library Panel has Wide and Narrow skeletons with Selector row, List controls
row, list, Hero header, and optional Workspace slots. It keeps one canonical
fixed-row browser (`MediaListCarrier<Target>`) across every geometry; geometry
changes clamp the viewport in place. In non-Wide geometry a hero-bearing browser
opens a Library Hero overlay on demand. The Panel derives skeleton and Hero
header shape from shared geometry and content policy; destinations choose
neither.

Workflow: identify the root-composed Panel; project content into its typed
slots; let the Panel arrange and paint; retain pointer geometry in the painter;
translate slot intents at the destination boundary. Verify one painter per
surface at Wide and Narrow, including absent Panels.

## Reuse workflow

1. **Look for an existing component or arrangement first** in
   `crates/mbv-render/src/components.rs` and `arrangements.rs` — a row, card,
   modal frame, hero pane. For a hero-bearing surface that means
   `wide_hero_presentation`/`WideHeroPanes` (`arrangements/wide_hero.rs`) and
   `components/hero_model.rs` for content rows.
2. **If it almost fits, look for a policy or variant** (table below).
3. **If nothing fits, add centrally** — a new component/arrangement function or
   named variant, not inline in the screen.
4. **Last resort:** a named bespoke component (below).

## Where a difference lives

No row permits screen-owned geometry, raw Ratatui calls, or raw `Color`/`Style`
passed into a shared component.

| Kind of difference | Where it lives | Screen does |
|---|---|---|
| **Content** (title, metadata, rows, image) | The screen's typed content model | Populate fields; call the same component |
| **Named policy** (small closed set derived from shared state) | The owning Panel or shared component | Nothing — derived centrally, never a caller-only choice |
| **New component** (general need, no painter fits) | A new function in `components/` or `arrangements/`, like `modal_frame::render_modal_frame` | Call it |
| **Bespoke surface** (reuse fails after a real attempt) | A named bespoke component with its reason and its own buffer test | Call it; it still obeys ownership, theming, and test rules |

Examples:

- Different modal subtitle → content: pass it in the modal's content model; no
  `subtitle_color: Option<Color>` parameter.
- Focused vs muted row → named policy: derive the style once in the owning
  painter (e.g. `components/list_rows.rs`), not
  `if focused { palette::X } else { palette::Y }` in a screen.
- A destination wants a different Hero arm → defect: derive it from shared
  content policy in the Library Panel.
- New side-by-side sizing rule → extend the owning Panel or arrangement, not a
  local `Layout` call.

## What the compiler does not catch

Only one rule is compiler-enforced: the `Slot` enum, the `Theme` value,
and `active()` are crate-private to `crates/mbv-theme/` — consumers reach
colour only through a `Role` or a surface row. Everything else is review's
job, and a green build proves
none of it:

- a `screens/` module importing ratatui, building a `Rect`, calling
  `render_widget` or `buffer_mut()`, or a literal `Color::Rgb(..)` anywhere;
- a background fill taken from a role instead of a surface row, or a second
  role or surface row for a concept that already has one;
- a second near-identical arrangement instead of extending the first;
- a `sync_*`/push helper carrying component-local state back into `App`;
- hit-test arithmetic that no longer matches painted geometry;
- `App`, a Service client, `PlayerProxy`, `Config`, or a channel reaching a
  component.

## Tests

Presentation tests are owned by the narrowest layer that can regress the fact
(`docs/invariants/14-test-ownership.md`). A new TUI test owns a contract no
existing test owns, or reproduces a real bug.

| Layer | Owned proof | Claims removed from this layer |
|---|---|---|
| Arrangement | Relational placement and breakpoint decisions | Painted glyphs, shell state, absolute coordinates |
| Render Component | Focused buffer content and paint-local geometry | Whole application frames and parent placement |
| Interactive Component | Local state transitions, semantic requests, viewport, retained hit resolution | Root placement and shell effects |
| Shell tick integration | Mount/focus/subscription/routing, latest-frame delivery, projection, cross-boundary effects | Glyph choice, spacing, repeated row arithmetic |

- Pane order and size: assert once, relationally, in the arrangement test
  (`hero.x == browser.right() + gap`), never as absolute coordinates.
- A removed assertion names its surviving owner, or records that the detail is
  no longer a contract.
- Literal terminal dimensions only when size is the input under test; otherwise
  derive capacity from named constants plus slack.
- Semantic glyph assertions only in the owning painter's test; no whole-frame
  equality or snapshots.
- Mounting/focus/subscription/routing changes need real `Application::tick()`
  tests (`src/app/tests/tick_integration/`) through the shell sync pass; direct
  `Component::on` tests don't verify composition. These assert composition and
  delivery, never painter appearance.

## Completion checklist

- [ ] No ratatui/`Rect`/`render_widget`/`buffer_mut()` added to `screens/`.
- [ ] Checked at the narrow breakpoint, not only default width.
- [ ] Hit-geometry arithmetic still matches any moved/resized painting.
- [ ] Nothing shell-owned reached a component; boundary crossings are typed `Msg`s.
- [ ] No chord resolved outside `src/app/input/`.
- [ ] One owner and one painter per surface per breakpoint.
- [ ] Each assertion sits in its owning test layer; removed assertions name
      their surviving owner.
- [ ] No absolute pane coordinates/widths or left/right ordering asserted.
- [ ] A focused buffer test covers changed output (added first, in its own
      commit, when output should not change); no checker, snapshot, or live
      fixture added.
