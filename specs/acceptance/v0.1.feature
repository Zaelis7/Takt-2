@v0.1
Feature: Takt 0.1 API-first monitoring core
  Takt must provide a reliable single-instance monitoring workflow that behaves
  consistently on PostgreSQL and SQLite.

  Background:
    Given a fresh Takt 0.1 instance
    And the default organization and project exist
    And I am authenticated as the local administrator

  @PRD-MON-001 @PRD-API-001
  Scenario: Create and manage a monitor through the public API
    When I create an HTTP monitor with slug "public-api" through POST /api/v1
    Then the response status is 201
    And the response contains a UUID id and version 1
    And the Location and ETag headers are present
    And the monitor is visible through the UI and taktctl

  @PRD-API-003
  Scenario: Repeating a create request is idempotent
    Given I created a monitor using idempotency key "test-key-0001"
    When I repeat the identical request with idempotency key "test-key-0001"
    Then I receive the same monitor id
    And only one monitor and one create audit event exist
    When I reuse the key with a different request body
    Then the response status is 409
    And the problem code is "idempotency_key_reused"

  @PRD-API-003
  Scenario: An outdated client cannot overwrite a monitor
    Given two clients read monitor "public-api" at version 1
    And the first client updates it with If-Match version 1
    When the second client updates it with If-Match version 1
    Then the response status is 412
    And the problem code is "version_conflict"
    And the first update remains stored

  @PRD-MON-002 @PRD-ALT-002
  Scenario: HTTP target failures and recovery follow configured thresholds
    Given monitor "public-api" has failure threshold 3 and recovery threshold 2
    And its controlled target is healthy
    When two consecutive target timeouts occur
    Then monitor "public-api" is not DOWN
    When a third consecutive target timeout occurs
    Then monitor "public-api" becomes DOWN once
    And one alert event is placed in the outbox
    When two consecutive successful checks occur
    Then monitor "public-api" becomes UP once
    And one recovery event is placed in the outbox

  @PRD-ALT-006
  Scenario: A database outage is never reported as a target outage
    Given monitor "public-api" is UP
    When the database becomes unavailable during result persistence
    Then no DOWN transition for "public-api" is stored or notified
    And readiness reports unavailable
    And a Takt infrastructure error is logged and counted
    When the database recovers
    Then the accepted observation is processed idempotently or visibly marked for retry

  @PRD-MON-002
  Scenario Outline: Every 0.1 check type produces a typed observation
    Given a valid <kind> monitor and a controlled successful target
    When a check is executed
    Then one observation with outcome "SUCCESS" is stored
    And its details contain no secret material

    Examples:
      | kind |
      | http |
      | tcp  |
      | dns  |
      | icmp |
      | tls  |
      | push |

  @PRD-AUT-001 @PRD-AUT-002
  Scenario: Declarative apply is stable and does not own manual resources
    Given a manual monitor "manual-check" exists
    And a valid config contains 500 declarative monitors
    When I validate, plan and apply the config
    Then the apply succeeds atomically
    And monitor "manual-check" is unchanged
    When I plan the unchanged config again
    Then the plan contains zero creates, updates and deletes

  @PRD-AUT-003
  Scenario: Prune only removes resources owned by the same source
    Given sources "source-a" and "source-b" each manage a monitor
    And a manual monitor exists
    When source "source-a" applies an empty config with prune enabled
    Then only the monitor managed by "source-a" is deleted
    And the source-b and manual monitors remain unchanged

  @PRD-NOT-002 @PRD-NOT-004
  Scenario: A notification secret is masked and delivery is retried
    Given a webhook channel has a secret signing key
    When I read, export, audit and test the channel
    Then the secret value appears in none of the responses or logs
    When the target responds 500 twice and 204 on the third attempt
    Then the same event id is delivered three times with increasing backoff
    And the delivery is marked successful

  @PRD-STA-002
  Scenario: Uptime is time weighted and excludes unknown and maintenance
    Given a monitor was UP for 40 minutes, DOWN for 10 minutes, UNKNOWN for 5 minutes and in MAINTENANCE for 5 minutes
    When I request uptime for the full 60 minutes
    Then the included duration is 50 minutes
    And the uptime ratio is 0.8
    And the excluded ratio is approximately 0.1666667

  @PRD-STA-001 @PRD-STA-006
  Scenario: A public status page contains only its public projection
    Given a public status page contains monitor "public-api"
    And the monitor uses an internal probe id, secret header and private diagnostic URL
    When an anonymous user fetches the public status API and page
    Then the name, public state and configured uptime are visible
    And the target URL, probe id, header, secret and internal error details are absent

  @PRD-IAM-001
  Scenario: An API token is scoped to monitor reads
    Given I create a token with only "monitors:read"
    When the token lists monitors
    Then the response status is 200
    When the token attempts to create a monitor
    Then the response status is 403
    And no monitor is created

  @PRD-DATA-001 @PRD-DATA-002 @PRD-DATA-004 @PRD-NFR-002
  Scenario Outline: Core persistence behavior is identical across databases
    Given Takt is configured with <database>
    When the migration and shared repository contract suites run
    Then migrations are forward-only and repeatable
    And an unknown newer schema is rejected while readiness remains unavailable
    And all repository contract cases pass
    And persistent ids, UTC timestamps and resource versions satisfy their shared contract

    Examples:
      | database   |
      | PostgreSQL |
      | SQLite     |

  @PRD-NFR-007
  Scenario: Backup and restore preserve monitoring data
    Given the instance contains monitors, observations, transitions and encrypted secrets
    When I create a supported backup
    And I restore it into a fresh instance with the required master key
    Then all resource ids and non-expired history are preserved
    And secrets can be used but never read back
    And the restored instance passes readiness

  @PRD-NFR-009
  Scenario: Primary UI flow works with keyboard and accessible names
    When I create, test and save an HTTP monitor using only the keyboard
    Then focus order is logical and visible
    And every input and status icon has an accessible name
    And the automated accessibility scan has no serious or critical violation
