# `v` selects queue artwork or the audio visualizer

The `v` key is reserved for selecting queue-card artwork or the audio visualizer, not for toggling Power View. Power View is controlled from the F2 settings surface, so `v` acts consistently on the queue card in every panel mode and playback context.

**Amended by ADR 0013 (2026-07-24):** this ADR's stated premise — "Power View remains a persisted view setting, not the default view" — no longer holds; Power View is now the only view. The conclusion is unaffected and in fact simplified: `v` remains reserved for the audio visualizer, now unconditionally rather than context-dependently, and the "transient fullscreen visualizer outside Power View" surface described here is no longer reachable. Only the embedded surface remains.

**Amended (2026-08-21):** `v` no longer toggle-enables a separate embedded visualizer area. The visualizer shares the queue card's artwork rectangle, so `v` selects between the current queue artwork and the visualizer in every panel mode and every playback context (including attached Emby sessions, where the selection applies but PipeWire capture still never starts). Selecting artwork stops any active capture. There is no longer a separate visualizer placement below the queue list or in wide queue-only playback-panel leftovers.

**Amended (2026-08-22):** the artwork/visualizer selection is session-local. Every launch starts on artwork; the selection is not persisted and a stale `visualizer_enabled` prefs key from earlier versions is ignored.

The embedded surface is backed by a supervised PipeWire capture worker. PipeWire reads the default system output; enabling it does not reroute mpv or modify the persistent system audio graph. The selection is session-local — every launch starts on artwork — and PipeWire capture is only started for playback audible on this machine: local playback, same-host Local daemon playback, or Direct remote Player-owner playback (whose audio may be forwarded into the local system output, e.g. by Snapcast). It is never started for attached Emby Session playback or audio-pipe playback.
