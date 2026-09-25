## ADDED Requirements

### Requirement: Explicit refresh refetches the Newest Episodes shelf

An explicit refresh of an Audiobookshelf podcast library SHALL re-request that library's `Newest Episodes` shelf alongside its shows, and SHALL NOT request shelves for any other library. The result SHALL replace that library's Latest items only when it belongs to the current Service setup generation. Until a successful result arrives, and when the refetch fails, Latest SHALL keep showing the previously loaded shelf. The refresh and the replacement SHALL NOT change the library's selected pill or restore an acknowledged Latest marker.

#### Scenario: Refresh updates Latest

- **WHEN** the user refreshes a podcast library and the server's Newest Episodes shelf has changed since it was loaded
- **THEN** that library's Latest pill shows the refetched shelf without restarting the client

#### Scenario: Refresh is scoped to its library

- **WHEN** the user refreshes one of several podcast libraries
- **THEN** only that library's shelf is re-requested

#### Scenario: Stale shelf result after Service replacement

- **WHEN** a shelf refetch initiated for a previous setup generation arrives after Service replacement
- **THEN** mbv SHALL ignore it and Latest SHALL keep its current items

#### Scenario: Failed refetch keeps the previous shelf

- **WHEN** the shelf refetch fails
- **THEN** Latest SHALL continue to show the previously loaded items

#### Scenario: Latest selection survives refresh

- **WHEN** the Latest pill is selected and acknowledged, and the user refreshes the library
- **THEN** Latest remains the selected pill after the refetched shelf lands and its marker stays cleared
