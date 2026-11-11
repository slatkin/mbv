# Spec Delta

## ADDED Requirements

### Requirement: Bindings are editable at runtime under the load rules

A runtime edit of the bindings (a router chord, a prefix-namespace chord, the prefix chord, or a reset to the declared default) SHALL be accepted only when the whole resulting configuration passes every rule that the configuration load applies. An accepted edit SHALL replace the running bindings of the Client that made it and SHALL be saved to `[keys]` at once. A rejected edit SHALL change neither the running bindings nor the file.

#### Scenario: An accepted edit is saved

- **WHEN** a Client accepts a rebind of `next_track` to a free chord
- **THEN** the Client fires `next_track` on that chord, and `[keys]` in the file holds it under the `next_track` action's section

#### Scenario: A rejected edit changes nothing

- **WHEN** a Client rejects an edit because the chord collides with another action's effective chord
- **THEN** the running bindings and `[keys]` in the file are both unchanged

#### Scenario: A reset removes the override

- **GIVEN** `seek_back` has an override in `[keys]`
- **WHEN** a Client resets `seek_back` to its default
- **THEN** `[keys]` holds no entry for `seek_back`, and an all-default configuration leaves no `[keys]` table

#### Scenario: The file loads back to the same bindings

- **WHEN** a Client saves an accepted edit and the configuration is loaded again
- **THEN** the loaded bindings equal the running bindings that the Client saved

### Requirement: A settings save leaves the key bindings as the file has them

A save of the other settings SHALL NOT write `[keys]`. Only an accepted runtime binding edit SHALL write it. A Client that loaded its bindings before another Client's edit SHALL NOT put back its own copy when it saves its settings.

#### Scenario: A TUI settings save keeps a GUI rebind

- **GIVEN** a TUI Client started, and later the GUI Client saved a rebind of `toggle_mute`
- **WHEN** the TUI Client saves a settings change
- **THEN** `[keys]` in the file still holds the GUI Client's rebind of `toggle_mute`
