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
collapsed or expanded state SHALL change only through `x` and the F2 Panel width rows; a resize
that mbv did not request SHALL NOT change it. The three-state `x` cycle and the mini-view `x`
toggle SHALL NOT be reachable while pinned. Outside the pinned panel this requirement SHALL NOT
apply.

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

#### Scenario: Resize from outside mbv

- **WHEN** mbv runs in the pinned panel at its collapsed width, a compositor key resizes the panel
  to `width_expanded`, and the user then presses `x`
- **THEN** the panel stays at `width_expanded` and the layout shows library-only with library
  focus; the next `x` collapses the panel

#### Scenario: The cycle is unreachable while pinned

- **WHEN** mbv runs in the pinned panel and the user presses `x` any number of times
- **THEN** the three-state cycle and the mini-view toggle never run; the layout is always one of
  the two pinned views
