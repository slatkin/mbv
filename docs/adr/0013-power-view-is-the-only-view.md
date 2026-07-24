# Power View Is The Only View

mbv had two full UIs: a tab-based Standard view and Power View, a two-column layout with a persistent queue column beside a library browser. Power View had become the one that was actually developed and used, while Standard survived mainly as the automatic fallback for terminals with no image protocol. We deleted Standard entirely (~2600 lines of render code, the `ViewMode` enum, `tab_idx`, and the `start_on_queue` setting) and made Power View unconditional.

## Considered Options

- Keep Standard as an automatic no-images fallback. Rejected because Power View already guards every image call behind `images_enabled()` and degrades to text and placeholders on its own, so the fallback bought a second full UI to maintain in exchange for a graceful degradation we already had. The cost was ongoing: every feature had to be built, tested, and visually verified twice, and in practice the Standard path was the one that silently rotted.
- Build a dedicated text-only layout that reclaims the card area when images are off. Rejected for now as speculative — nobody has reported the placeholder layout as a problem. Worth revisiting if images-off turns out to be a real usage mode rather than a theoretical one.
- Make Power the default but leave Standard reachable. Rejected because it is not a decision, it is a deferral: the maintenance cost stays and the ambiguity about which view is canonical stays with it.

## Consequences

There is no fallback UI. If Power View breaks on some terminal, mbv is broken on that terminal — there is no second path to fall back to. Images-off is therefore a first-class supported configuration that must be visually verified on changes to the render tree, not an edge case.

Because there is no longer a second view to contrast against, "Power" stopped being meaningful as a name and the vocabulary was renamed in the same change: queue side and library side, per `CONTEXT.md`'s existing guidance. The old names had also drifted into contradiction — `power_left_*` meant the queue column while `PowerFocus::Left` meant the library side.

Library positions were previously kept in two isolated scopes, one per view. That collapsed to one. The saved positions from both scopes were discarded rather than merged: any merge rule would have been arbitrary, and the cost of being wrong is one browsing session that starts at a library root.

`v` and `g` became unbound. `v` stays reserved for the audio visualizer (ADR 0009) rather than being reused.

## Implementation note (2026-07-24)

The rename landed as its own commit (`b51cb82`) separate from the deletion (`860e672`), specifically so a behavioral regression and a rename mistake would never be indistinguishable in one diff. It surfaced two things worth recording:

- A blind identifier rename briefly renamed the *string literals* in the prefs-file backward-compatibility fallback (`"power_focus"` etc., intentionally left as the old on-disk key names so a pre-upgrade `prefs.json` still loads), which would have silently broken that compatibility path had it not been caught by manual review. A rename this size is not risk-free just because it is "mechanical."
- Not every `power_`-prefixed identifier was renamed — only the ones this decision's scope covered (the queue/library-side naming collision). Names like `render_power_home_list` remain, and are not evidence of an incomplete rename so much as evidence that "power" survives as an implementation detail (nerd-font "powerline" styling, for instance) even though it no longer names a view.
