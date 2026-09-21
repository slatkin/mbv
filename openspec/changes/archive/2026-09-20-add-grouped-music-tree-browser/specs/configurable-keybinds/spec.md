## Purpose

Covers the help sidebar's chord-column automeasure that landed with the 2026-09-20 tweak batch (user-approved as necessary): the Help chord column is measured from the rendered rows themselves, so a configured rebind can no longer overflow into the label column.

## ADDED Requirements

### Requirement: The help chord column is measured from the rendered rows

The help sidebar SHALL size its chord column from the rows it renders: the width SHALL be the widest
rendered chord plus a two-column gap before the label column, floored at the historical alignment
width so short binding sets keep their existing look. The measurement SHALL come from the rendered
rows themselves — built once with padding disabled — not from a second hand-written chord table, so
any configured or declared rebind (for example `Ctrl+Left / Ctrl+Right`) is measured the same way as
the defaults and can never run the chord column into the label column.

#### Scenario: A long rebound chord no longer overflows

- **WHEN** a configured chord renders wider than the historical column floor
- **THEN** the chord column widens to that chord plus the two-column gap
- **AND** no help row's label starts inside the chord column in any section

#### Scenario: Short default bindings keep the historical alignment

- **WHEN** every rendered chord in the loaded keybind set fits within the historical column width
- **THEN** the chord column keeps the historical width
- **AND** the help presentation is unchanged from the historical alignment
