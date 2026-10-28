# emby-remote-play Specification

## Purpose

Defines how mbv handles a Play command that an Emby remote-control session sends over the
websocket, and which local UI state that command must leave alone.

## Requirements

### Requirement: A remote Play leaves local UI interaction unchanged

When an Emby remote-control session sends a Play command that plays now, mbv SHALL replace the
Owner queue and start playback, and it SHALL NOT change local UI interaction: panel focus, panel
mode, and the pinned panel width SHALL stay as they were. Toasts that report the play or its
failure SHALL still show.

#### Scenario: A remote Play keeps panel focus

- **WHEN** the library panel has focus and an Emby remote-control session sends a Play command
  that plays now
- **THEN** the Owner queue is replaced, playback starts, and the library panel keeps focus

#### Scenario: A remote Play keeps the expanded pinned view

- **WHEN** mbv runs in the pinned panel at its expanded width and an Emby remote-control session
  sends a Play command that plays now
- **THEN** playback starts, no pinned panel request is sent, and the panel keeps its expanded
  width and library view
