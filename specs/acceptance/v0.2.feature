@v0.2
Feature: Takt 0.2 distributed and operational monitoring
  Remote locations and incident operations must add evidence without confusing
  a probe outage with a target outage.

  Background:
    Given a running Takt 0.2 server upgraded from a valid 0.1 fixture
    And I am an administrator of the default organization

  @PRD-MON-007
  Scenario: Enroll a probe with a one-time code
    Given I create a probe enrollment that expires in 15 minutes
    When a new probe submits a valid CSR and the enrollment code
    Then it receives a certificate bound to its probe id
    And the probe connects using mutual TLS
    When a second client reuses the enrollment code
    Then enrollment is rejected
    And the rejection is audited and rate limited

  @PRD-MON-007
  Scenario: A client without a valid probe certificate cannot receive jobs
    When an unauthenticated client opens the probe control endpoint
    Then the TLS or authentication handshake fails
    And no job metadata or secret material is returned

  @PRD-MON-007 @PRD-ALT-006
  Scenario: An offline probe does not make a healthy target DOWN
    Given a monitor is UP and assigned only to probe "berlin"
    When probe "berlin" disconnects beyond the stale window
    Then the location and monitor become UNKNOWN
    And no target DOWN alert is created
    And the UI explains that evidence is missing because the probe is offline

  @PRD-MON-007
  Scenario: A probe queues observations during a server disconnect
    Given a probe accepted jobs before the control connection was interrupted
    When the server connection is unavailable for 15 minutes
    And the probe completes 100 accepted jobs
    Then all 100 observations are stored in the encrypted offline queue
    When the connection recovers
    Then all 100 observations are delivered and acknowledged exactly once by id
    And the local queue removes them only after acknowledgement

  @PRD-MON-007
  Scenario: Duplicate and late observations have at most one state effect
    Given a job has already produced a canonical evaluation
    When two duplicate observations and one late observation arrive for that job
    Then at most one evaluation can change the current monitor state
    And every discarded or late result has a visible reason code

  @PRD-MON-007
  Scenario: Quorum distinguishes target failures from missing evidence
    Given a quorum monitor selects three probes and requires two successes
    When two probes succeed and one reports a target failure
    Then the monitor is UP or DEGRADED according to its tolerance
    When two probes report target failures and one succeeds
    Then the monitor is DOWN after its failure threshold
    When one probe succeeds and two probes are offline
    Then the monitor is UNKNOWN

  @PRD-ALT-005
  Scenario: Maintenance suppresses alerts but preserves observations
    Given an active maintenance marks monitor "database" as MAINTENANCE
    When a real target failure occurs
    Then the observation and evaluation are stored
    And no failure notification is delivered during maintenance
    When maintenance ends and a new target failure still meets the threshold
    Then the monitor becomes DOWN and a new alert can be delivered

  @PRD-ALT-005
  Scenario: Flapping is visible and notifications are throttled
    Given a monitor changes between UP and DOWN at least four times in ten windows
    When the flapping rule activates
    Then the current truthful state remains visible
    And repeated notifications are throttled
    And a flapping event explains the throttle

  @PRD-STA-003 @PRD-STA-004
  Scenario: Publish and resolve a manual incident
    When an editor publishes an incident with an initial INVESTIGATING update
    Then the incident appears on the selected status page
    When the editor adds IDENTIFIED, MONITORING and RESOLVED updates
    Then the public timeline preserves all updates in order
    And previously published updates cannot be edited in place

  @PRD-MIG-001 @PRD-MIG-002 @PRD-MIG-004
  Scenario: Import supported Uptime Kuma data without silent loss
    Given a supported Uptime Kuma export with monitors, tags, notifications and status pages
    When I run import analyze
    Then supported, transformed and unsupported fields are listed
    When I run import plan and apply
    Then supported resources are created with stable origin metadata
    And missing secrets are visibly marked as required configuration
    When I repeat the same import
    Then no duplicate resource is created

  @PRD-NFR-007
  Scenario: Upgrade from 0.1 preserves ids and history
    Given a supported 0.1 database fixture with encrypted secrets
    When Takt 0.2 performs its documented migration
    Then every existing resource id, state transition and retained observation is preserved
    And local checks continue after readiness becomes healthy
    And a backup can restore the pre-upgrade state if migration fails
