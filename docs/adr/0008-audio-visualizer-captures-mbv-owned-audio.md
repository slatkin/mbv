# Audio visualizer captures mbv-owned audio

mbv's audio visualizer is scoped to PulseAudio/PipeWire-pulse systems and must visualize mbv playback audio only, not the default system monitor. The first capture strategy may route mpv through a dedicated Pulse-compatible sink and read that sink's monitor, but any accepted strategy must preserve normal playback feel: noticeable audio latency or A/V sync regression is a blocker to troubleshoot and pivot around, not an acceptable tradeoff.

**Considered Options**

- Capture the default monitor source: rejected because it can include unrelated application audio.
- Depend on mpv visualization video filters: rejected because they render visualization as video inside mpv rather than exposing samples for mbv's TUI.
- Treat ALSA/no-Pulse systems as first-class targets: rejected for the initial design to keep per-app audio isolation precise.
