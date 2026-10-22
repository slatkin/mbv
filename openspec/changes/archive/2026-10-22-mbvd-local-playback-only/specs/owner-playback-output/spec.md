# Spec Delta

## MODIFIED Requirements

### Requirement: The packaged owner has only local playback

Packaged `mbvd` SHALL have local playback as its only output. Its runtime setup SHALL have no way to express a remote output: no Cast receiver attachment and no Emby Session output can be configured for the packaged role, so the exclusion does not rest on a value that the packaged process could set differently. Packaged `mbvd` SHALL NOT advertise a remote-output ctrl capability, a Client connected to it SHALL NOT offer a remote output, and it SHALL reject a request to select one with a stated reason while local playback stays the output.

#### Scenario: Client connected to mbvd

- **WHEN** a Client is connected to a packaged `mbvd` owner
- **THEN** it does not offer a Cast receiver or an Emby Session as an output

#### Scenario: Request reaches mbvd

- **WHEN** a request to select a Cast receiver or an Emby Session reaches packaged `mbvd`
- **THEN** `mbvd` rejects it with a stated reason and keeps local playback as the output

#### Scenario: Packaged hello carries no remote-output capability

- **WHEN** packaged `mbvd` greets a ctrl Client
- **THEN** its hello carries neither the Cast playback-output capability nor the Emby Session output capability

#### Scenario: Packaged runtime setup has no remote-output choice

- **WHEN** the packaged daemon is started
- **THEN** its runtime setup cannot be given a Cast receiver attachment or an Emby Session output in any build of the packaged program
