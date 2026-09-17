## Why

On a grouped canonical media list the first selectable row sits below its artist/letter `Heading`.
The window-raise rule moves the window top exactly to the selected display row, so after scrolling
back up the cursor rests on the group's first item and its `Heading` stays one row above the window
— the top row of the list never comes back (issue #731, Music album rail). This is the third
recurrence of the same missing-row class: the legacy power-view painter was patched for it
(`510fe59`), and the canonical owner reimplemented the rule without that patch.

The reported symptom is keyboard scrolling; the rule is shared by every input, so fixing it in the
raise rule fixes it for all of them without touching any binding.

## What Changes

- The shared media-list owner's window-raise rule, when it raises the window to bring the selection
  back into view, continues raising over the contiguous non-selectable rows (`Heading`/`Spacer`)
  directly above the selection, so the label of the group containing the cursor stays visible.
- Nothing else. The wheel, `PgUp`/`PgDn`, and every cursor chord keep their current meaning; the
  window gains no new state, no new writer, and no independent scroll verb.

Not in scope (rejected by the user, 2026-09-17): a viewport-step model, wheel rebinding,
`PgUp`/`PgDn` page redefinition, list-local scroll chords, and window-as-owner-state-with-one-writer.
The implementation attempted on `feat/media-list-viewport-scroll` was abandoned unmerged.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `canonical-media-lists`: the window-raise rule keeps a group's label above its first selected item.
