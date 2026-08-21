---
name: mbv-frontend
description: Ownership rules and workflow for mbv's terminal UI (src/app/render/). Use before adding or changing rendering code in a screen, before adding a new visual variant, and before reporting a TUI change complete.
---

# mbv-frontend

`src/app/render/` is split into four kinds of module. The split is the
enforcement boundary from `openspec/changes/enforce-mbv-ui-design-system`; the
term definitions live in `CONTEXT.md` under Presentation and are authoritative
if this skill and `CONTEXT.md` ever disagree.

| Module | Owns | Must not |
|---|---|---|
| `screens/` | app state in, typed content model out | call Ratatui, construct a `Rect`, compute a hit target |
| `arrangements/` | placement of components within a `Rect`, breakpoints | own painting or app state |
| `components/` | painting, its own geometry within a `Rect` | take arbitrary `Color`/`Style` from a screen |
| `theme/` | semantic roles (public) | expose raw `Color` primitives (private) |

Dependency order: `screens -> arrangements -> components -> Ratatui`.

Not every surface has been migrated to this yet —
`openspec/changes/enforce-mbv-ui-design-system/ledger.md` tracks what remains.
An existing violation in a file you are not touching is not licence to add a
new one anywhere, including in that same file.

## Reuse workflow

Before writing rendering code for a screen:

1. **Look for an existing component or arrangement first.** Check
   `src/app/render/components/mod.rs` and `src/app/render/arrangements/mod.rs`
   for something that already paints this shape (a row, a card, a modal
   frame, a hero pane). Reuse it before writing a new painter.
2. **If it almost fits, check for a policy or variant** (see the decision
   table below) before reaching for a screen-local branch.
3. **If nothing fits, add centrally** — a new component/arrangement function,
   or a new named variant/policy on an existing one — not inline in the
   screen.
4. **If it genuinely cannot use the shared vocabulary**, register it as a
   named bespoke component (see below). This is the last resort, not the
   default when reuse looks inconvenient.

## Controlled-override decision table

None of these rows permit screen-owned geometry, raw Ratatui calls, or raw
`Color`/`Style` values passed into a shared component. The only question is
*where* the difference lives.

| Kind of difference | Where it lives | Screen does |
|---|---|---|
| **Content change** (different title, metadata, rows, image) | The screen's own typed content model | Populate the model's fields; call the same component/arrangement |
| **Named policy** (a small closed set of valid style/behaviour combinations already exists) | The component/arrangement that defines the policy | Select the named policy constructor, e.g. a focus/unfocus style pair like `list_rows::focused_or_muted(focused)` |
| **Central variant** (a new but still centrally-owned presentation, e.g. Inline hero vs. Hero-on-left) | The owning arrangement or component, as a new named variant | Select the variant; never paint the alternate presentation itself |
| **New component** (no existing painter fits, but the need is general) | A new function in `components/` or `arrangements/`, exposed like `modal_frame::render_modal_frame` | Call the new component; the component is reviewable and reusable by other screens |
| **Bespoke surface** (reuse genuinely does not fit after a real attempt) | A named bespoke component, with its stated reason and its own buffer coverage | Call the bespoke component; it still obeys ownership, semantic theming, and verification rules — it is not exempt from them |

### Worked examples

- *"This screen needs a different subtitle on the modal."* Content change.
  Pass the subtitle into the existing modal's content model; do not add a
  `subtitle_color: Option<Color>` parameter to `render_modal_frame`.
- *"This row should look focused or muted depending on state."* Named policy.
  Use the existing `focused_or_muted`/`focused_or_subtle` style pair in
  `components/list_rows.rs` rather than inlining
  `if focused { palette::X } else { palette::Y }` in the screen.
- *"This browse surface wants hero-on-left instead of inline hero."* Central
  variant. Both presentations already exist in `arrangements/hero_left.rs`
  and `components/hero.rs`; the screen selects which one applies (per the
  width/height gate), it does not build a third layout.
- *"Nothing existing places two panes side by side with this sizing rule."*
  New component/arrangement. Add the placement function to `arrangements/`
  (or extend an existing one with a named variant if the shape is close
  enough) — not a one-off `Layout::horizontal([...])` call inside the screen.
- *"This surface's presentation is genuinely unlike anything else in the
  app."* Bespoke surface. Register it as a named bespoke component with the
  reason written down and its own buffer test; it still may not call Ratatui
  from the screen module, and it still consumes theme roles, not raw colours.

## Ratatui patterns

```rust
// Wrong: screen calls Ratatui directly.
// src/app/render/screens/some_screen.rs
f.render_widget(Paragraph::new(text), Rect { x, y, width, height });

// Right: screen builds a content model, component paints it.
// src/app/render/screens/some_screen.rs
let model = SomeRowModel { text, focused };
self.render_some_row(f, area, &model); // defined in components/

// src/app/render/components/some_component.rs
pub(in crate::app::render) fn render_some_row(f: &mut Frame, area: Rect, model: &SomeRowModel) {
    let fg = focused_or_muted(model.focused); // named policy, not a raw Color
    f.render_widget(Paragraph::new(model.text.clone()).style(Style::default().fg(fg)), area);
}
```

```rust
// Wrong: screen splits its own layout.
let [left, right] = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(area);

// Right: an arrangement owns the split, screen calls it.
let (left, right) = hero_on_left_panes(area); // arrangements/hero_left.rs
```

```rust
// Wrong: screen picks an arbitrary colour.
Style::default().fg(palette::ACCENT_ACTIVE).bg(Color::Rgb(20, 20, 20))

// Right: screen passes semantic focus state; the component resolves the role.
// (Raw Color primitives are private to theme/ and cannot be named outside it.)
Style::default().fg(if focused { palette::ACCENT_ACTIVE } else { palette::TEXT_PRIMARY })
```

## What these checks do not catch

Three mechanisms enforce this boundary, in descending strength. Know which
one you're relying on:

1. **The compiler** — private theme primitives. A raw `Color` outside
   `theme/` is a compile error. Cannot be bypassed.
2. **ast-grep**, scoped to `src/app/render/screens/`
   (`rules/frontend-boundary/*.yml`, run via `ast-grep scan` from repo root) —
   flags `use ratatui::`, `render_widget`/`render_stateful_widget`,
   `Layout::...`, `Rect` construction, and `buffer_mut()` in screen modules.
   This catches the common bypass and nothing subtler. It does **not** catch:
   - **Duplicated arrangement geometry** — a screen that calls an existing
     arrangement correctly but a second, near-identical arrangement was added
     elsewhere instead of extending the first one.
   - **Hit targets drifted from painting** — a component's geometry changes
     but the corresponding coordinate arithmetic in `src/app/input_mouse*.rs`
     is not updated to match.
   - Test files (`*tests*.rs`) and inline `#[cfg(test)] mod tests { ... }`
     blocks inside an otherwise-production file are not distinguished by
     these rules; a `#[cfg(test)]` block that legitimately builds a
     `TestBackend` buffer will still be flagged if it lives in a
     non-`*tests*`-named file. Prefer a dedicated `..._tests.rs` file for new
     buffer tests so the check stays accurate.
3. **Review**, against the checklist below — this is what catches the two
   items above. A clean ast-grep run is not proof of conformance.

## Completion checklist

Before reporting a TUI change complete:

- [ ] **Component ownership** — no `use ratatui::`, `render_widget`, `Layout::`,
  `Rect` construction, or `buffer_mut()` was added to a `screens/` module. If
  ast-grep flags something you added, fix it rather than widening an `ignores`
  glob.
- [ ] **Narrow-width behaviour** — the change was checked at the narrow/mini
  breakpoint, not only the default width.
- [ ] **Interaction targets** — if painted geometry moved or resized, the
  corresponding hit-target arithmetic in `input_mouse*.rs` still matches it.
- [ ] **Buffer tests** — a characterization test exists (or was added first,
  in its own commit, per the ledger migration flow) and passes unchanged
  where the change is not expected to alter output.
