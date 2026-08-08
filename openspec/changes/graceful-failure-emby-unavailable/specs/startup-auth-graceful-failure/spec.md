## Purpose

Defines the startup authentication behavior when the Emby server is unreachable, including timeout bounds, failure classification, token preservation, and user-facing feedback.

## ADDED Requirements

### Requirement: Startup authentication wall-clock bound is 5 seconds

The startup authentication path SHALL complete or fail within a 5-second wall-clock bound. This bound is independent of the underlying HTTP client's timeout configuration and serves as a hard ceiling on how long mbv waits before declaring the server unreachable.

#### Scenario: Server is responsive

- **WHEN** the Emby server is reachable and responds within 5 seconds
- **THEN** authentication SHALL succeed normally
- **THEN** the TUI SHALL start without delay beyond normal authentication time

#### Scenario: Server is unresponsive

- **WHEN** the Emby server is unreachable or unresponsive
- **WHEN** 5 seconds have elapsed since the authentication attempt began
- **THEN** the authentication attempt SHALL be abandoned
- **THEN** mbv SHALL proceed to failure handling without waiting for the underlying HTTP client

### Requirement: Connectivity-class failures exit to command line

When startup authentication fails due to network connectivity issues, mbv SHALL print a clear error message to stderr and exit with a non-zero status code. The login screen SHALL NOT be shown for connectivity-class failures.

#### Scenario: Connection timeout

- **WHEN** the Emby server does not respond within the 5-second bound
- **THEN** mbv SHALL print an error message to stderr indicating the server could not be reached
- **THEN** mbv SHALL exit with a non-zero status code
- **THEN** the login screen SHALL NOT appear

#### Scenario: Connection refused

- **WHEN** the Emby server actively refuses the connection
- **THEN** mbv SHALL print an error message to stderr indicating the server could not be reached
- **THEN** mbv SHALL exit with a non-zero status code
- **THEN** the login screen SHALL NOT appear

#### Scenario: DNS resolution failure

- **WHEN** the Emby server URL cannot be resolved
- **THEN** mbv SHALL print an error message to stderr indicating the server could not be reached
- **THEN** mbv SHALL exit with a non-zero status code
- **THEN** the login screen SHALL NOT appear

### Requirement: Error message format follows existing patterns

The error message printed on connectivity failure SHALL follow the format used by other startup errors in `main.rs`: a concise statement of what failed, with no recovery instructions or suggestions. The message format SHALL be `mbv: could not reach {server_url}: {error_details}`.

#### Scenario: Timeout error message

- **WHEN** authentication fails due to timeout
- **THEN** the error message SHALL include the server URL and indicate a timeout occurred
- **THEN** the message SHALL NOT include instructions such as "please log in again" or "check your configuration"

#### Scenario: Connection refused error message

- **WHEN** authentication fails due to connection refused
- **THEN** the error message SHALL include the server URL and indicate the connection was refused
- **THEN** the message SHALL follow the concise pattern established by other startup errors

### Requirement: Token preservation on connectivity failures

When startup authentication fails due to connectivity issues, the cached authentication token SHALL be preserved. The token SHALL only be cleared on authentication-class failures (401/403 responses indicating invalid or expired credentials).

#### Scenario: Server comes back after temporary outage

- **WHEN** startup authentication fails due to connectivity issues
- **WHEN** the user restarts mbv after the Emby server becomes available
- **THEN** mbv SHALL use the previously cached token
- **THEN** the user SHALL NOT be prompted to re-authenticate

#### Scenario: Credentials are actually expired

- **WHEN** the Emby server responds with 401 or 403 indicating the token is invalid
- **THEN** the cached token SHALL be cleared
- **THEN** the login screen SHALL appear to allow re-authentication

### Requirement: Login screen only shown for credential issues

The login screen SHALL appear only when the user must authenticate with credentials. It SHALL NOT appear for connectivity failures. The login screen SHALL be shown when:
- The cached token is expired or rejected (401/403)
- No cached credentials exist (first run)
- The server URL is missing from configuration

#### Scenario: First run with no cached credentials

- **WHEN** no cached credentials exist
- **THEN** the login screen SHALL appear
- **THEN** the user SHALL be able to enter server URL, username, and password

#### Scenario: Token expired

- **WHEN** the cached token is rejected with 401 or 403
- **THEN** the login screen SHALL appear
- **THEN** the user SHALL be able to re-authenticate

#### Scenario: Missing server URL

- **WHEN** no server URL is configured
- **THEN** the login screen SHALL appear
- **THEN** the user SHALL be able to enter the server URL

#### Scenario: Server unreachable with valid cached credentials

- **WHEN** the Emby server is unreachable
- **WHEN** valid cached credentials exist
- **THEN** the login screen SHALL NOT appear
- **THEN** mbv SHALL exit to command line with an error message

### Requirement: Failure classification distinguishes connectivity from authentication

The system SHALL classify authentication failures into two categories: connectivity-class and authentication-class. Connectivity-class failures include timeouts, connection refused, DNS errors, and TLS errors. Authentication-class failures include 401 and 403 responses. Each category SHALL trigger different handling as specified in the requirements above.

#### Scenario: Network error is connectivity-class

- **WHEN** the underlying error is a network timeout, connection refused, DNS failure, or TLS error
- **THEN** the failure SHALL be classified as connectivity-class
- **THEN** mbv SHALL exit to command line with an error message
- **THEN** the cached token SHALL be preserved

#### Scenario: HTTP 401 is authentication-class

- **WHEN** the Emby server responds with HTTP 401
- **THEN** the failure SHALL be classified as authentication-class
- **THEN** the cached token SHALL be cleared
- **THEN** the login screen SHALL appear

#### Scenario: HTTP 403 is authentication-class

- **WHEN** the Emby server responds with HTTP 403
- **THEN** the failure SHALL be classified as authentication-class
- **THEN** the cached token SHALL be cleared
- **THEN** the login screen SHALL appear
