## ADDED Requirements

### Requirement: Library Wide lists stripe with the library secondary pair

Both library Wide lists (the Browser-pane list and the provider workspace list) SHALL paint zebra striping with secondary focused `#48584e` and secondary unfocused `#2d353b`, following the same stripe semantics as the queue list: the alternate background applies to qualifying visible selectable `Item` rows by screen-row parity among items, headings and spacers never carry it, and the selected row always shows the selected-row treatment instead of the stripe. Zebra striping SHALL remain a per-list policy — these colours apply to the library Wide lists only and change nothing on any other list.

#### Scenario: Library stripes match the queue pattern in library colours
- **WHEN** a library Wide list renders four visible selectable items with focus
- **THEN** the striped positions carry the `#48584e` background and the unstriped positions carry no secondary background, at the same screen-row positions the queue list stripes

#### Scenario: Unfocused library stripes use the unfocused secondary
- **WHEN** a library Wide list renders while unfocused
- **THEN** the striped positions carry the `#2d353b` background

#### Scenario: Library headings never stripe
- **WHEN** a Heading or Spacer row appears between two library items
- **THEN** it paints with no zebra background and does not shift the item stripe pattern

### Requirement: Wide selected row uses the gutter treatment with no exceptions

Every Wide presentation SHALL render its selected row with the gutter treatment: the selected title paints bold in the focus-accent role with the row's default background — no selected-row background fill and no marker glyph. There SHALL be no per-list opt-in or opt-out; the treatment is the Wide default for the queue list, both library Wide lists, and any future Wide consumer. An unfocused list SHALL show no selection mark at all (zebra and ordinary row colours only). Multi-selected rows on an unfocused list SHALL keep their existing multi-selection presentation.

#### Scenario: Selected row has no background fill
- **WHEN** a Wide list renders its selected row while focused
- **THEN** the title paints bold in the focus-accent role
- **AND** the row background is the same as an unselected row in that position
- **AND** no icon or marker glyph appears in or beside the row

#### Scenario: Unfocused list hides selection
- **WHEN** a Wide list renders while unfocused
- **THEN** no row carries the bold focus-accent title
- **AND** striped positions still show the unfocused secondary background

#### Scenario: Selected row overrides its stripe
- **WHEN** the selected row falls on a striped screen-row position
- **THEN** the gutter treatment applies and the zebra background does not
