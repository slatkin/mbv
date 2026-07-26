## ADDED Requirements

### Requirement: Direct remote active pill styling
The queue header MUST render the active remote pill in a direct mbv-to-mbv remote queue with yellow foreground (`#dbbc7f`) on an aqua background (`#35a77c`). The inactive direct-remote pill styling and the interactive local/remote split MUST remain unchanged.

#### Scenario: Active direct remote scope
- **WHEN** the application is connected to another mbv app and the remote queue scope is active
- **THEN** the remote pill uses `#dbbc7f` foreground and `#35a77c` background
- **AND** the local and remote pills remain the interactive scope controls

#### Scenario: Inactive direct remote scope
- **WHEN** the application is connected to another mbv app and the local queue scope is active
- **THEN** the remote pill retains the existing inactive styling and remains selectable

### Requirement: Attached non-mbv display pill
When the application is connected to an attached non-mbv client such as emby, the queue header MUST render a single right-side remote pill. The pill MUST use the existing remote icon, MUST show the attached session device name when present, and MUST fall back to the session host name when the device name is absent. It MUST use foreground `#1e2326` and yellow background `#dbbc7f`.

#### Scenario: Attached session with device name
- **WHEN** an attached mbv-to-emby session is connected and has a non-empty device name
- **THEN** the queue header shows one right-side pill with the existing remote icon and that device name
- **AND** the pill uses `#1e2326` foreground on `#dbbc7f` background

#### Scenario: Attached session host fallback
- **WHEN** an attached mbv-to-emby session is connected and its device name is absent or empty
- **THEN** the queue header shows the session host name in the right-side pill
- **AND** the existing remote icon and exact attached-session colors are retained

#### Scenario: Direct remote does not render attached display treatment
- **WHEN** the connection state is a direct mbv-to-mbv remote
- **THEN** the queue header renders the local/remote interactive split rather than the attached-session display-only pill

### Requirement: Attached display pill is non-interactive
The attached non-mbv queue-header pill MUST NOT be a queue-scope target. Clicking it, focusing it through queue-scope keyboard behavior, or dispatching a scope-selection action at its location MUST be a no-op and MUST NOT change queue scope, route, connection state, or attached-session queue behavior.

#### Scenario: Mouse click on attached pill
- **WHEN** the user clicks within the attached display pill
- **THEN** input dispatch performs no queue-scope action and application state remains unchanged

#### Scenario: Keyboard or action attempt on attached pill
- **WHEN** keyboard navigation or a queue-scope action targets the attached display pill
- **THEN** no focusable/selectable attached target is exposed and no state or queue behavior changes

#### Scenario: Direct remote controls remain interactive
- **WHEN** the connection state is a direct mbv-to-mbv remote
- **THEN** the existing local and remote queue-scope hitboxes and keyboard actions continue to select their respective scopes

### Requirement: Queue header layout remains stable
The queue header MUST reserve and render the attached display pill on the right without overlapping the queue title or existing direct-remote interactive areas. Label width, clipping, and narrow-terminal behavior MUST be deterministic and MUST preserve the current attached-session queue behavior.

#### Scenario: Attached pill fits available width
- **WHEN** an attached session is connected and the queue header has sufficient width
- **THEN** the pill is rendered on the right side with its icon and resolved host/device label visible

#### Scenario: Attached pill exceeds available width
- **WHEN** the resolved attached-session label is longer than the available right-side header width
- **THEN** the label is clipped or truncated using the existing header width rules without overlap or a new interactive hitbox

#### Scenario: No remote connection
- **WHEN** no direct remote or attached session is connected
- **THEN** the queue header retains its existing local-only layout and dimensions
