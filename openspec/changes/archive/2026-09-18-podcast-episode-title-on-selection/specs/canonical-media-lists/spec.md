# canonical-media-lists

## ADDED Requirements

### Requirement: Lists may reveal their rows' titles on selection

The shared list owner SHALL accept a closed title-reveal policy for its rows, defaulting to every row painting its full title. When a list declares the reveal-on-selection policy:

- A row that is not part of the list's current selection SHALL paint only its primary (context) text. No fragment of the secondary title SHALL appear in that row, and the row SHALL NOT reserve the hidden title's width.
- A row that is part of the list's current selection — the cursor row, or a multi-selected row — SHALL paint its full context-and-title text.
- The reveal on its own SHALL NOT animate anything: a selected row's title marquees exactly according to the selected-row marquee requirement, which this policy extends.

The policy SHALL be declared once by the destination that composes the list and resolved at the shared row-paint seam. No destination SHALL paint its own rows, fork the row painter, or pass a per-row flag to express the policy, and no list that never declares it SHALL change behaviour.

#### Scenario: Unselected rows hide the item title

- **WHEN** a reveal-on-selection list paints a row that is not part of its current selection
- **THEN** that row paints its primary context text only
- **AND** no part of the row's secondary title appears, with no ellipsis standing in for it

#### Scenario: The selected row reveals the item title

- **WHEN** a reveal-on-selection list paints its cursor row on an unfocused list
- **THEN** that row paints its full context-and-title text
- **AND** the title renders static, truncated at the row's title slot if it does not fit

#### Scenario: Lists that never declare the policy are unchanged

- **WHEN** a list composes rows without declaring a title-reveal policy
- **THEN** every row paints its full title exactly as before this change, including lists whose rows carry a secondary title

## MODIFIED Requirements

### Requirement: Selected row marquees an overflowing title instead of truncating

When the shared row painter renders the row that is both the list's current selection and on a focused list, and that row's title does not fit its title slot, the painter SHALL animate the title through a bounded back-and-forth marquee (hold at the start, scroll to reveal the tail, hold at the end, scroll back) rather than ellipsis-truncating it. A list that declares its rows reveal their title only on selection SHALL marquee that selected row's title whether or not it fits its slot, carrying the title fully out of the window and back instead of holding a fitting title static. Every other row — unselected rows, the selected row on an unfocused list, and any row whose title already fits its slot in a list that reveals titles on every row — SHALL continue to render with ellipsis truncation exactly as before this change. The marquee's timing SHALL match the existing player-strip title marquee's cadence (used for the Now Playing and idle-feed titles), so the two forms of marqueeing feel identical to the user.

Each list owns an independent marquee clock (mirroring its ownership of cursor and scroll). The clock SHALL key on the full text it marquees — both parts of a split row's title, not the primary text alone — and SHALL restart from the beginning whenever that text changes: a new row becomes selected, or the selected row's own title text changes. A freshly-marqueed title therefore never opens mid-scroll, and two rows that share a primary text never share a clock position.

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

- **WHEN** the selected, focused row's title already fits its slot and its list reveals titles on every row
- **THEN** it renders in full, static, with no marquee and no ellipsis

#### Scenario: A fitting title in a reveal-on-selection list marquees

- **WHEN** the selected, focused row's title already fits its slot and its list reveals titles only on selection
- **THEN** the title marquees through the same cycle, scrolling fully out of the window and back
- **AND** it does not sit static or ellipsis-truncated

#### Scenario: Marquee restarts on a new selection

- **WHEN** the cursor moves to a different row — including one whose context text matches the previous row's — or the selected row's title text itself changes
- **THEN** that row's marquee begins again from its held starting position rather than resuming mid-cycle
