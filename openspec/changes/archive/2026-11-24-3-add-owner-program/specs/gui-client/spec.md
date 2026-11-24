# Spec Delta

## MODIFIED Requirements

### Requirement: The GUI program ships beside the terminal program

Every release artifact (the tarball, the `.deb`, and the Arch packages) SHALL install the `mbv-gui` program beside the `mbv` program and the `mbv-owner` program, and SHALL declare the Vulkan loader and fontconfig runtime dependencies. No artifact SHALL declare a WebKitGTK dependency. The GUI Client finds and starts its Owner from the sibling `mbv-owner` program, and the version and wire handshake refuses a mismatched pair, so all three programs SHALL come from one build.

#### Scenario: A package installs both programs

- **WHEN** the tarball, the `.deb`, or an Arch package is installed
- **THEN** `mbv`, `mbv-gui`, and `mbv-owner` are installed in the same directory

#### Scenario: A package declares the render stack

- **WHEN** the `.deb` or an Arch package is inspected
- **THEN** it depends on the Vulkan loader and fontconfig
- **AND** it does not depend on WebKitGTK

### Requirement: The GUI Client attaches to the Owner process as a Client

The GUI Client SHALL be a Client of the Owner process under the same rules as a TUI Client. It SHALL find, start, and attach to the Owner process as a TUI Client does. It SHALL NOT hold the owner lock. It SHALL NOT become a Player owner. When the Owner process exits, the GUI Client SHALL follow `daemon-disconnect-handling`: an announced shutdown SHALL end the run, and an unannounced connection loss SHALL present the recovery dialog.

#### Scenario: No Owner process is running

- **WHEN** the GUI Client starts and no Owner process is running
- **THEN** the GUI Client starts the Owner process from the `mbv-owner` program and attaches to it

#### Scenario: A TUI Client and a GUI Client are attached at once

- **GIVEN** Stay-alive is enabled and a TUI Client is attached
- **WHEN** the GUI Client starts
- **THEN** it attaches alongside the TUI Client without displacing it
- **AND** both show the same Bound queue and playback state

#### Scenario: Stay-alive is disabled and a Client is attached

- **GIVEN** Stay-alive is disabled and a TUI Client is attached
- **WHEN** the GUI Client starts
- **THEN** the GUI Client refuses to start with the same explanation a second TUI Client gets

#### Scenario: The Owner process is from a different build

- **GIVEN** a running Owner process from a different build
- **WHEN** the GUI Client starts
- **THEN** the window shows the mismatch and offers Restart and Quit, each on a key
- **AND** Restart stops that Owner process, starts a new one from the `mbv-owner` program, and attaches to it

#### Scenario: Unannounced Owner loss presents the recovery dialog

- **GIVEN** the GUI Client is attached to the Owner process
- **WHEN** the connection closes with no announced shutdown
- **THEN** the window shows the recovery dialog with the last playing title, the daemon log path, and the Restart and resume, Restart, don't resume, and Quit choices, each on its key
- **AND** no other key and no press outside the dialog answers it, and the GUI Client does not exit on its own
