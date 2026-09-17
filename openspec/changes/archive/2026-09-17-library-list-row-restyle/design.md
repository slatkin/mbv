## Context

The shared media-list row painter (`render/components/media_list/row.rs`) resolves both the
right-aligned duration slot and the split-row secondary title. Every library owner projects a
duration (`home_content`, `podcast_content`, `tv_content`, `music_content`, `book_content`,
`feeds_content`); the Queue list is the only other duration producer today, and the sessions modal
paints its own time. The split-row secondary title currently resolves `PLAYBACK_TITLE_FG`, the
playback strip's own-name role.

## Goals / Non-Goals

**Goals:**

- Library browse rows project no duration, with no new painter flag or caller-selected arm.
- The split-row item title gets its own sage role so the playback strip's aqua title is untouched.

**Non-Goals:**

- Changing the Queue list's duration, the sessions modal's `pos / dur`, the playback strip's
  elapsed/pos-dur transport, or the hero meta rows' duration (a hero is not a list).
- Per-list palette opt-ins: the split-row role stays unconditional for any split-row producer.

## Decisions

### D1: Drop the duration at the owners, not the painter

Each library owner stops computing and projecting its duration (`duration: None`) and deletes the
now-dead formatting call. The painter and the row model keep the duration slot for the Queue list.

*Alternatives considered:* a "library" painter flag threaded from the Library panel (rejected: a
caller-selected arm, and the row model already carries duration as content); removing the field and
duplicating a queue-only model (rejected: churn for one remaining consumer).

### D2: A new `SPLIT_ROW_TITLE_FG` sage role

Add `SPLIT_ROW_TITLE_FG = primitives::IRIS` (sage, `#A7C080`) and resolve the split-row secondary
title through it. The playback strip keeps `PLAYBACK_TITLE_FG` (aqua) through `title_part_fg`.

*Alternatives considered:* changing the `PLAYBACK_TITLE` primitive (rejected: moves the strip the
user did not ask to move); reusing `DURATION` or `ACCENT_ACTIVE` (rejected: unrelated meanings, and
`DURATION` now belongs to the Queue list's slot).

## Risks / Trade-offs

- [A future library list re-projects a duration] → the `canonical-media-lists` clause states the
  duration slot is queue-only, so adding one back reopens the contract.
