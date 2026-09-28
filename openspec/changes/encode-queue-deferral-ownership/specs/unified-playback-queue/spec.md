# Spec Delta

## ADDED Requirements

### Requirement: A queue replacement deferred behind the unsaved-playlist prompt runs only through that prompt's answer

When a queue replacement would discard unsaved changes to the queue's saved playlist, the system SHALL hold the replacement and ask the user to save, discard, or cancel. Discard SHALL run the held replacement immediately. Cancel SHALL drop it. Save SHALL start one playlist save and bind the held replacement to that save. The held replacement SHALL run only when that bound save completes successfully for the same queue and playlist. If the bound save fails or cannot start, the held replacement SHALL be dropped. No other playlist save SHALL run it, including an automatic save after consume, a save on quit, or a later manual save. If the prompt is closed without an answer, the held replacement SHALL never run. Replacements held by the populated-queue confirmation and by the "play locally instead" prompt SHALL run only from their own prompt's confirmation.

#### Scenario: Save answer runs the replacement after its save

- **WHEN** the user answers Save on the unsaved-playlist prompt
- **AND** the save it started completes successfully
- **THEN** the held replacement SHALL run

#### Scenario: Prompt closed without an answer

- **WHEN** the unsaved-playlist prompt is closed by something other than the user's answer
- **AND** a later automatic save of the same playlist completes successfully
- **THEN** the held replacement SHALL NOT run

#### Scenario: Another save completes first

- **WHEN** the user answers Save while an earlier save of the same playlist is still in flight
- **AND** that earlier save completes successfully
- **THEN** the held replacement SHALL NOT run until the save started by the answer completes

#### Scenario: Bound save fails

- **WHEN** the save started by the Save answer fails
- **THEN** the held replacement SHALL NOT run
- **AND** a later successful save of the same playlist SHALL NOT run it

#### Scenario: Save completion with a gated replacement outstanding

- **WHEN** a replacement awaits the populated-queue confirmation
- **AND** a playlist save completes
- **THEN** the gated replacement SHALL NOT run until the user confirms that prompt
