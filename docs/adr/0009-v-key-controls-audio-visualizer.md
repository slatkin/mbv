# `v` controls the audio visualizer

The `v` key is reserved for showing or toggling the audio visualizer, not for toggling Power View. Power View remains a persisted view setting, not the default view, and is controlled from the F2 settings surface, so `v` can act consistently as the visualizer command: embedded enable/disable in Power View, transient fullscreen visualizer entry outside Power View.

**Amended by ADR 0013 (2026-07-24):** this ADR's stated premise — "Power View remains a persisted view setting, not the default view" — no longer holds; Power View is now the only view. The conclusion is unaffected and in fact simplified: `v` remains reserved for the audio visualizer, now unconditionally rather than context-dependently, and the "transient fullscreen visualizer outside Power View" surface described here is no longer reachable. Only the embedded surface remains.
