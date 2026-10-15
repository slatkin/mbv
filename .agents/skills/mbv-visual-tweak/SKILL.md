---
name: mbv-visual-tweak
description: Make a small appearance-only change in the mbv TUI. Use when the user asks to change a colour, zebra stripe, palette value, hero or header styling, pill, gutter, glyph, icon, spacing, or a text format in the mbv terminal UI. Triggers include real phrasings such as "make the TRACKLIST header FOAM", "add a new palette called Clay", "zebra to #2e383c", "change the tree view colour back", "one space between the home icon", "duration should be sage". Do not use for behaviour or layout logic; those follow the OpenSpec workflow.
---

# mbv visual tweak

This skill covers appearance-only changes to the mbv TUI: colour, spacing,
glyphs, and text format. It does not cover behaviour. A behaviour change
follows the OpenSpec workflow.

## Read first

1. `docs/architecture/tui-frontend.md`. It owns the theming, ownership, and
   test rules. This skill adds only the colour workflow.
2. `CONTEXT.md` if you name a domain concept.
3. `crates/mbv-theme/src/lib.rs`. Its doc comment states the colour authority.

## Scope check first

- One value or one surface changes → do the tweak directly.
- A colour scheme must change across several surfaces → this is a design
  decision. Propose an OpenSpec change instead.
- The request adds behaviour, a new surface, or layout logic → OpenSpec
  workflow, not this skill.

## The colour authority chain

Colour resolves in one direction only:

1. A painter or screen names a meaning: a `Role` variant for foreground, a
   `Surface` variant for a background fill. Painters never name a hue.
2. `Role` and `Surface` rows map to a `Slot`.
3. `Slot` and `Theme` are crate-private to `crates/mbv-theme`.
4. `Theme::DEFAULT` in `crates/mbv-theme/src/slot.rs` is the only place a
   `Color::Rgb` literal lives.

Hard rules the compiler does not catch:

- No `Color::Rgb` literal outside `Theme::DEFAULT`.
- No raw `Color` or `Style` passed into a shared component.
- No `if focused { X } else { Y }` colour choice in a screen. The owning
  painter derives that policy.

## Map the user's words before you edit

The user speaks in colour words and hex values: "foam", "sage", "ink",
"mauve", "Clay", `#2e383c`, "Grey5". `docs/palette.json` holds the current
value of every slot, role, and surface.

1. Find the painted site. Search the owning painter or component.
2. Map the user's word to the meaning it names. Ask: does this change what
   one meaning looks like, or what one slot looks like everywhere?
3. State the mapping in one line before you edit. Example: "sage → the
   Green slot; `Role::WorkspaceHeaderFg` keeps its meaning". Then proceed
   unless the user objects.

## Change recipes

### Recolour one meaning

The user renames or restyles one meaning, for example "the duration text is
sage now".

- Keep the role. The role's slot decides the hue.
- If another slot already paints that hue, re-point the role to that slot in
  `crates/mbv-theme/src/role.rs`.
- If no slot has that hue, change the slot's colour in `Theme::DEFAULT`. This
  changes every use of that slot. Check `docs/palette.json` first for other
  users of the slot, and name them to the user if they shift.

### Change one surface fill

Background fills resolve through the surface table. Edit the surface's resting
or focused slot in `crates/mbv-theme/src/surface_table.rs`. Never fill a
background from a `Role`.

### Add a new colour

A new hue means a new `Slot` variant. This widens a closed set:

1. Add the variant to `Slot`, extend the `Theme` storage, and set its colour
   in `Theme::DEFAULT`.
2. Run the theme tests. `palette_json.rs` regenerates `docs/palette.json` on
   every test run and prints a stderr hint for a new entry.
3. Fill the empty `uses` prose for the new entry in `docs/palette.json` by
   hand. The generator never writes prose.
4. Never hand-edit the mechanical fields of `docs/palette.json`. Edit the
   theme crate and let the test rewrite the file.

### Share a style that two painters duplicate

If two painters hard-code the same choice, give the concept one role or one
surface row, then use it at both sites. Never add a second role or surface row
for a concept that already has one.

### Spacing, glyphs, and text format

Edit the owning painter or component. One owner and one painter per surface
per breakpoint. If you move or resize painted rows, hit-test geometry must
match the new painted geometry.

## Tests

`tui-frontend.md` owns the test rules and the completion checklist; follow
them. For a tweak they mean in practice:

- A focused buffer test covers the changed output, added with the change. If
  output must not change, add the test first, in its own commit.
- Semantic glyph assertions go only in the owning painter's test.
- Check the narrow breakpoint too, not only default width.

## Verify

```bash
cargo check -p <package>
cargo nextest run -p <package>
cargo fmt
```

Colour is judged by eyes, not by tests. Build and launch mbv, then ask the
user to verify the change on screen. Live testing belongs to the user; ask
before you run anything live. Commit the change; never leave a dirty
worktree.
