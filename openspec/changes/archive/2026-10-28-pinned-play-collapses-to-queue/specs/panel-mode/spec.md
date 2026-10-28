# Spec Delta

## MODIFIED Requirements

### Requirement: Pinned launches bind x to the pinned views

When mbv runs in the pinned panel, pressing `x` SHALL toggle the panel between its two pinned
views: from the collapsed width (`width`) it SHALL expand the panel to `width_expanded` over the
tiled windows and show library-only with library focus; from the expanded width
(`width_expanded`) it SHALL collapse the panel to `width` and show queue-only with queue focus
(in mini view when the collapsed panel is below 80 columns). Each press SHALL change the width
and the covering mode in one panel change. If the panel rejects the target width, a warning toast
SHALL name the reason and neither the width nor the displayed panel mode SHALL change. The
collapsed or expanded state SHALL change only through `x`, the F2 Panel width rows, and the
collapse that `A play from the expanded pinned view collapses to the queue view` starts; a resize
that mbv did not request SHALL NOT change it. Each press SHALL send at most one panel request,
off the TUI thread. While a request is pending, a further `x` press SHALL be ignored, with no
queue. The outcome SHALL set the view on success or flash the reason on failure; a refusal SHALL
leave both the width and the displayed panel mode unchanged. The three-state `x` cycle and the
mini-view `x` toggle SHALL NOT be reachable while pinned. Outside the pinned panel this
requirement SHALL NOT apply.

#### Scenario: x expands to the library view

- **WHEN** mbv runs in the pinned panel at its collapsed width and the user presses `x`
- **THEN** the panel animates to `width_expanded` pixels over the tiled windows, no tiled window moves, and the layout shows library-only with library focus

#### Scenario: x collapses to the queue view

- **WHEN** mbv runs in the pinned panel at its expanded width and the user presses `x`
- **THEN** the panel collapses to `width` pixels, no tiled window moves, and the layout shows queue-only with queue focus

#### Scenario: Collapsed queue view is mini view at the default width

- **WHEN** the collapsed panel is below 80 columns and the user presses `x` to collapse from the expanded width
- **THEN** the collapsed panel shows the queue-only mini view with queue focus

#### Scenario: Rejected expanded width leaves the view alone

- **WHEN** mbv runs in the pinned panel at its collapsed width, `x` is pressed, and `width_expanded`
  would leave the output no width
- **THEN** a warning toast names the reason, the panel keeps its collapsed width, and the displayed
  panel mode is unchanged

#### Scenario: x while a request is pending

- **WHEN** mbv runs in the pinned panel and the user presses `x` again before the first request finishes
- **THEN** the second press is ignored, no second request is sent, and the view follows the first request's outcome

#### Scenario: Resize from outside mbv

- **WHEN** mbv runs in the pinned panel at its collapsed width, a compositor key resizes the panel
  to `width_expanded`, and the user then presses `x`
- **THEN** the panel stays at `width_expanded` and the layout shows library-only with library
  focus; the next `x` collapses the panel

#### Scenario: The cycle is unreachable while pinned

- **WHEN** mbv runs in the pinned panel and the user presses `x` any number of times
- **THEN** the three-state cycle and the mini-view toggle never run; the layout is always one of
  the two pinned views

## ADDED Requirements

### Requirement: A play from the expanded pinned view collapses to the queue view

When mbv runs in the pinned panel at its expanded width and a user action starts playback now,
mbv SHALL collapse the panel to its queue view, as `x` does from the expanded width. The collapse
SHALL start only after the Owner accepts the queue replacement, and at most one time per action.
It SHALL apply to every output. Enqueue, a refused or cancelled play, and a play at the collapsed
width SHALL NOT change the view. A refused collapse SHALL NOT stop playback.

#### Scenario: Play collapses the expanded panel

- **WHEN** mbv runs in the pinned panel at its expanded width and the user plays an item, plays
  all, or shuffles from the library
- **THEN** playback starts, the panel collapses to `width`, and the layout shows queue-only with
  queue focus

#### Scenario: Play to another output collapses the panel

- **WHEN** mbv runs in the pinned panel at its expanded width, a Cast receiver or a directly
  controlled Emby session is the playback target, and the user plays from the library
- **THEN** the play goes to that target and the panel collapses to its queue view

#### Scenario: A multi-item play collapses one time

- **WHEN** mbv runs in the pinned panel at its expanded width and the user plays a multi-item
  selection whose first item starts playback and whose other items append
- **THEN** mbv sends one collapse request

#### Scenario: Enqueue keeps the expanded view

- **WHEN** mbv runs in the pinned panel at its expanded width and the user enqueues an item
- **THEN** the item appends to the queue and the panel stays at its expanded width with the
  library view

#### Scenario: A cancelled replace keeps the expanded view

- **WHEN** mbv runs in the pinned panel at its expanded width, the user plays into a populated
  queue, and cancels the replace confirmation
- **THEN** nothing plays and the panel stays at its expanded width with the library view

#### Scenario: A rejected replace keeps the expanded view

- **WHEN** mbv runs in the pinned panel at its expanded width, the user plays from the library,
  and the Owner does not apply the replacement
- **THEN** no collapse request is sent and the panel stays at its expanded width

#### Scenario: A refused collapse does not stop playback

- **WHEN** a play from the expanded pinned view starts playback and the panel refuses the
  collapsed width
- **THEN** a warning toast names the reason, the panel keeps its expanded width and library view,
  and playback continues

#### Scenario: A pending pinned request blocks the collapse

- **WHEN** a pinned panel request is pending and a play from the expanded pinned view starts
  playback
- **THEN** mbv sends no second request and the view follows the pending request's outcome

#### Scenario: Unpinned play is unchanged

- **WHEN** mbv is not pinned and the user plays from the library
- **THEN** the panel mode does not change because of the play
