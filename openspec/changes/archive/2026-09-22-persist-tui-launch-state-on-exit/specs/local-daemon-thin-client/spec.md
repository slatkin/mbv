# Spec Delta

## MODIFIED Requirements

### Requirement: Session continuity is not provided
mbv SHALL NOT reconstruct a Client's complete on-screen session across that Client exiting. Scroll offsets, open overlays and dialogs, in-flight searches, multi-selection, nested Workspace selections, Queue selection, and the queue undo history SHALL reset in a newly started Client.

A newly started Client SHALL restore only the same bounded TUI launch-state snapshot as Bare mode: the selected tab, that tab's selected main Selector pill and selected library item, and Panel focus, with the current-content fallbacks defined by the `tui-launch-state` capability. This bounded launch location is not full Session continuity and SHALL NOT reintroduce a terminal-multiplexing layer.

#### Scenario: A client exits with UI state on screen
- **WHEN** a Client with an open overlay, an active search, multi-selection, a scrolled list, and a selected Queue item exits
- **WHEN** the user starts mbv again
- **THEN** the new Client SHALL start with no overlay, no active search, no multi-selection, default scroll state, and no restored Queue selection
- **THEN** playback SHALL be unaffected

#### Scenario: Persisted state still returns
- **WHEN** a Client exits normally with a selected tab, main Selector pill, library item, and Panel focus
- **WHEN** the user starts mbv again
- **THEN** the new Client SHALL restore that bounded launch location exactly as Bare mode does
- **THEN** missing identities SHALL use the current-content fallbacks defined by the `tui-launch-state` capability
