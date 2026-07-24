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
- Not every `power_`-prefixed identifier was renamed — only the ones this decision's scope covered (the queue/library-side naming collision). Names like `render_power_home_list` remain, and are not evidence of an incomplete rename so much as evidence that "power" survives as an implementation detail (nerd-font "powerline" styling, for instance) even though it no longer names a view. Separately, the rename's claim of "zero behavior change" needs one carve-out: the image-cache-suffix values changed (`"P"`→`"card"`, `"pwr_al"`→`"album_card"`, and the `"lib"` suffix was dropped entirely), which is an intentional, accepted one-time cache-key migration — see decision 10 — but is a genuine one-time runtime difference (existing cached art re-downloads once), not literally zero behavior change.
- The queue-side/library-side rename is incomplete in a way that reintroduces the exact naming collision this decision set out to fix, at smaller scale. Four fields on `LayoutMain` still carry the ambiguous `left` name from before this decision: `left_area`, `left_row_map`, `left_row_targets`, and `left_sorted_indices` — all mean the *library* side (confirmed by `render/mod.rs` tests asserting `layout.left_area.x >= queue_column_width`). Meanwhile `render_main`'s own *local* variables named `left_area`/`left_w` mean the *queue* column (keyed off `queue_column_collapsed`). Both meanings of `left_*` now coexist inside the same function. `actions.rs`'s `lib_page_size()` reads `self.layout.main.left_area.height` under a comment claiming it's "the right panel" — accurate in meaning, confusing in wording, given the field's name. None of this is a behavior bug; it is exactly the kind of naming trap this ADR exists to prevent, left half-fixed. Follow-up: rename the queue-side locals in `render_main` to a `queue_*` family and the `LayoutMain` fields to a `library_*` family, so no `left_*` identifier remains anywhere in this subsystem.
