## Purpose

Define how mbv supplies and resolves the mpv Lua script set that renders its on-screen
control and prompt overlays, so that exactly one copy is live and the live copy can be
identified by the user.

## ADDED Requirements

### Requirement: Exactly one mpv script set is live

The player SHALL load exactly one copy of the mbv script set (the entry script and its
sibling fragments). A script copy left behind by a removed installer, a previous
version, or another build SHALL NOT take precedence over the copy belonging to the
running build, and a copy that is present but not in use SHALL NOT affect any overlay
the user sees.

#### Scenario: Leftover user-directory copy exists

- **WHEN** a script copy left by a removed installer exists outside the running build's own script set
- **AND** the player starts
- **THEN** the loaded script set SHALL be the one belonging to the running build
- **AND** the leftover copy SHALL NOT affect any overlay

#### Scenario: Running from a checkout

- **WHEN** the running build located next to a checkout provides the script set
- **THEN** that script set SHALL be the one loaded

#### Scenario: Installed build

- **WHEN** no checkout script set is available to the running build
- **THEN** the installed script set SHALL be loaded

### Requirement: The resolved script set is observable

Startup SHALL report the resolved script path, and SHALL warn - naming the path - when a
script copy in a removed installer's location exists but is not in use. mbv SHALL NOT
delete or rewrite such a copy.

#### Scenario: Resolved path reported

- **WHEN** the player starts and an mpv script set is loaded
- **THEN** startup SHALL report the resolved script path it handed to mpv

#### Scenario: Unused legacy copy is named

- **WHEN** a script copy in a removed installer's location exists and is not the resolved set
- **THEN** startup SHALL warn with that copy's path
- **AND** mbv SHALL leave that file untouched

### Requirement: The overlay script set and its fonts resolve under one rule

The mpv overlay script set and the font directory it references SHALL resolve under the
same rule, so a build never pairs its script set with another build's fonts.

#### Scenario: Fonts follow the script source

- **WHEN** the resolved script set comes from a checkout
- **THEN** the font directory handed to mpv SHALL come from the same checkout

#### Scenario: Leftover font directory does not win

- **WHEN** a font directory left by a removed installer exists outside the running build's own resources
- **THEN** the resolved font directory SHALL be the one belonging to the running build
