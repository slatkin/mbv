## ADDED Requirements

### Requirement: Relative navigation is resolved by the Player owner

Next and Previous SHALL be resolved by the Player owner against its canonical queue, whatever sent them: keyboard, mouse, MPRIS, the tray, or an Emby remote-control command. This SHALL hold whether the Player owner is the app process (Bare) or a daemon (Stay-alive, `mbvd`). The owner SHALL turn the step into a slot jump with its own request identity, dispatched and settled like any other slot jump. The Playback run SHALL NOT pick a neighbouring slot itself.

The neighbour SHALL be found from the latest requested target: the queued transition's target, else the in-flight transition's target, else the observed active slot. Next SHALL do nothing at the last slot and Previous SHALL do nothing at the first slot.

#### Scenario: MPRIS Next while attached to a Stay-alive daemon

- **WHEN** the TUI is attached to a Stay-alive daemon and the user invokes Next through MPRIS
- **THEN** the daemon's Player owner SHALL advance to the next canonical slot
- **AND** the request SHALL NOT be refused for lacking a ctrl wire form

#### Scenario: Emby remote Next under Stay-alive

- **WHEN** an Emby remote-control Next arrives at a Stay-alive daemon
- **THEN** the daemon's Player owner SHALL resolve the neighbour and dispatch a slot jump
- **AND** the target SHALL resume from the canonical queue

#### Scenario: Tray Next

- **WHEN** the user selects Next from the Local daemon's tray
- **THEN** the daemon's Player owner SHALL resolve and dispatch the step
- **AND** every attached client SHALL observe the resulting track change

#### Scenario: Bare keyboard Next

- **WHEN** the app process is the Player owner and the user presses Next
- **THEN** the app's Player owner SHALL resolve the neighbour and dispatch a slot jump with canonical resume

#### Scenario: Previous right after Next

- **WHEN** Next has been requested and not yet confirmed, and the user invokes Previous
- **THEN** Previous SHALL resolve from the requested Next target
- **AND** playback SHALL return to the slot that was playing before Next

#### Scenario: Next at the end of the queue

- **WHEN** the latest requested target is the last canonical slot and Next is invoked
- **THEN** no transition SHALL be dispatched

### Requirement: Recorded progress follows one rule per observation kind

Each Player owner and each client copy of the canonical queue SHALL decide the position to record for a finished occurrence using one rule per observation kind, and every copy SHALL reach the same result for the same observation:

- A **completion** observation (the occurrence ended and playback moved on) SHALL record position zero when the occurrence counts as played. Otherwise it SHALL record the observed position only for non-audio media at or beyond 30 seconds, and SHALL keep the previously recorded position in every other case.
- A **stop** observation (the user stopped playback) SHALL record position zero when the occurrence counts as played. Otherwise it SHALL record any positive observed position for non-audio media, and SHALL keep the previously recorded position for audio or a non-positive position.

#### Scenario: Video completes under 30 seconds

- **WHEN** a video occurrence with a recorded position of 20 minutes ends by moving on at 12 seconds without counting as played
- **THEN** every copy of the queue SHALL keep 20 minutes as that occurrence's position

#### Scenario: Audio completes mid-track

- **WHEN** an audio occurrence ends by moving on at 3 minutes without counting as played
- **THEN** every copy of the queue SHALL keep that occurrence's previous position

#### Scenario: Video stopped under 30 seconds

- **WHEN** the user stops a video occurrence at 12 seconds without it counting as played
- **THEN** every copy of the queue SHALL record 12 seconds for that occurrence

#### Scenario: Played occurrence

- **WHEN** an occurrence counts as played when its completion or stop is observed
- **THEN** every copy of the queue SHALL record position zero for it

### Requirement: A slot resumes the same however it is reached

The resume position used when an occurrence starts SHALL come from the Player owner's canonical queue, and SHALL be the same whether the occurrence is reached by Next, Previous, a direct jump, or an on-screen Next-Up accept. This applies to full-playlist and active-file playback alike. Audiobookshelf items SHALL resume from the Audiobookshelf playback session's position, which remains authoritative for that service.

#### Scenario: Previous back to a video left under 30 seconds

- **WHEN** a video occurrence with a recorded position of 20 minutes is played for 12 seconds, playback moves to the next slot, and the user invokes Previous
- **THEN** the video SHALL resume at 20 minutes, the same position a direct jump to that slot would use

#### Scenario: Previous back to an audio track

- **WHEN** an audio occurrence is left mid-track by Next and the user invokes Previous
- **THEN** it SHALL start from the same position a direct jump to that slot would use

#### Scenario: Non-Audiobookshelf item in an active-file queue

- **WHEN** a queue plays in active-file mode and the user jumps to a non-Audiobookshelf occurrence
- **THEN** that occurrence SHALL resume from the canonical queue's position for it
