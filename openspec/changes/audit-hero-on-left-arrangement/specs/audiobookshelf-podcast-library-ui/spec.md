## ADDED Requirements

### Requirement: The wide podcast right rail shows the same bucket pill bar as narrow

In the hero-on-left (wide) presentation, the podcast-show right rail SHALL show the
alphabetical show-title bucket pill bar above the show list, using the same pill
widget, labels, selected-bucket resolution, and `⌘` prefix as the narrow
presentation's pill bar. The pill bar SHALL NOT change between the wide and narrow
presentations; the wide right rail SHALL obtain the pill row from the shared
hero-on-left right-pane split rather than painting the show list directly onto the
pane.

#### Scenario: Podcast library is displayed wide

- **WHEN** an Audiobookshelf podcast library meets the wide geometry conditions
- **THEN** the show-title bucket pill bar renders at the top of the right rail
- **AND** the show list renders below it
- **AND** the pill bar is identical to the one shown in the narrow presentation

#### Scenario: Selected bucket in wide and narrow match

- **WHEN** the same podcast show is selected in the wide and the narrow
  presentation
- **THEN** the same bucket pill is marked selected in both
