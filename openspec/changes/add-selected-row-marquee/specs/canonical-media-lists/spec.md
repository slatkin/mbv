## ADDED Requirements

### Requirement: Selected row marquees an overflowing title instead of truncating

When the shared row painter renders the row that is both the list's current selection and on a focused list, and that row's primary text does not fit its title slot, the painter SHALL animate the title through a bounded back-and-forth marquee (hold at the start, scroll to reveal the tail, hold at the end, scroll back) rather than ellipsis-truncating it. Every other row — unselected rows, the selected row on an unfocused list, and any row whose title already fits its slot — SHALL continue to render with ellipsis truncation exactly as before this change. The marquee's timing SHALL match the existing player-strip title marquee's cadence (used for the Now Playing and idle-feed titles), so the two forms of marqueeing feel identical to the user.

Each list owns an independent marquee clock (mirroring its ownership of cursor and scroll). The clock SHALL restart from the beginning whenever the marqueed text changes — a new row becomes selected, or the selected row's own title text changes — so a freshly-marqueed title never opens mid-scroll.

#### Scenario: Focused selected row with an overflowing title marquees

- **WHEN** the row under the cursor on a focused list has a title wider than its available slot
- **THEN** the row's title animates through the hold/scroll/hold/scroll-back marquee cycle
- **AND** no ellipsis appears in the row while it is marqueeing

#### Scenario: Unfocused selection keeps truncating

- **WHEN** the row under the cursor has an overflowing title but its list is not focused
- **THEN** the row's title is ellipsis-truncated exactly as an ordinary row

#### Scenario: Non-selected rows keep truncating

- **WHEN** a row is not the list's current selection
- **THEN** its overflowing title is ellipsis-truncated regardless of focus

#### Scenario: Fitting title never marquees

- **WHEN** the selected, focused row's title already fits its slot
- **THEN** it renders in full, static, with no marquee and no ellipsis

#### Scenario: Marquee restarts on a new selection

- **WHEN** the cursor moves to a different row, or the selected row's title text itself changes
- **THEN** that row's marquee begins again from its held starting position rather than resuming mid-cycle
