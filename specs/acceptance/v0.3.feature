@v0.3
Feature: Takt 0.3 team and platform readiness
  Organizations, automation and browser checks must remain secure and auditable.

  Background:
    Given a running Takt 0.3 instance upgraded through the supported release path
    And organizations "alpha" and "beta" each contain a project named "production"

  @PRD-IAM-003 @PRD-IAM-004
  Scenario Outline: Roles enforce project permissions on the server
    Given user "alice" has role <role> in alpha production only
    When alice attempts action <action> in alpha production
    Then the response is <own_result>
    When alice attempts the same action in beta production using a known resource id
    Then the response is 404 or 403 without revealing the resource
    And no beta resource is changed

    Examples:
      | role     | action                    | own_result |
      | owner    | delete the project         | allowed    |
      | admin    | enroll a probe             | allowed    |
      | editor   | create a monitor           | allowed    |
      | operator | acknowledge an alert       | allowed    |
      | operator | change a monitor target    | denied     |
      | viewer   | read a monitor             | allowed    |
      | viewer   | create a silence           | denied     |

  @PRD-IAM-002
  Scenario: OIDC login uses code flow with PKCE and current group mapping
    Given an OIDC provider maps group "takt-operators" to operator
    When a user completes Authorization Code flow with valid PKCE, state and nonce
    Then a rotated Takt session with operator permissions is created
    When the provider removes the user from the mapped group
    And the session is refreshed or the configured revalidation interval expires
    Then the removed permissions are no longer usable
    And the change is audited

  @PRD-IAM-002
  Scenario: Invalid OIDC tokens are rejected
    When the callback contains a token with wrong issuer, audience, signature, nonce or expired time
    Then login fails generically
    And no session is created
    And no token contents are logged

  @PRD-STA-005
  Scenario: Status subscription requires double opt-in and supports unsubscribe
    When an anonymous user submits an email address to a public status page
    Then the address is pending and receives a single confirmation message
    And it receives no incident notifications yet
    When the user opens the valid confirmation link
    Then the subscription becomes active
    When the user opens its unsubscribe link
    Then future notifications stop without requiring login
    And public responses never reveal whether another address is subscribed

  @PRD-AUT-004
  Scenario: Terraform converges without reading secrets back
    Given Terraform defines an organization, project, monitor, channel, maintenance and status page
    When Terraform applies the configuration
    Then all resources exist with stable ids
    When Terraform plans again without a configuration change
    Then the plan is empty
    And secret values are absent from read responses, logs and provider diagnostics

  @PRD-AUT-005
  Scenario: Kubernetes discovery is opt-in and deletion has a grace period
    Given discovery has read-only access to namespace "payments"
    And an Ingress has the required Takt opt-in annotation
    When discovery completes
    Then a monitor is planned and created with stable origin metadata
    When the Ingress disappears for one discovery cycle
    Then the monitor is not deleted
    When it remains absent beyond the configured grace period
    Then deletion appears in a plan before it can be applied
    And resources without opt-in were never read into Takt resources

  Scenario: A monitor dependency suppresses only downstream notifications
    Given monitor "api" depends on monitor "database"
    And both monitors have genuine target failures
    When database becomes DOWN before api
    Then both monitors visibly remain DOWN
    And the api failure notification is suppressed with dependency "database" as reason
    And the database failure notification is delivered
    When I attempt to make database depend on api
    Then the cycle is rejected with a validation problem

  @PRD-MON-008
  Scenario: Browser checks run in an isolated worker
    Given a browser monitor attempts to navigate to a permitted controlled target
    When its declarative steps complete
    Then the observation contains only redacted timings and assertions
    And the temporary browser profile is deleted
    When a controlled page attempts to access a host file, control-plane address or metadata endpoint
    Then access is denied by the worker sandbox or egress policy
    And the Takt server process remains unaffected

  @PRD-MON-008 @PRD-ALT-006
  Scenario: A browser crash is not a target outage
    Given an otherwise healthy browser target
    When Chromium crashes before target evidence is obtained
    Then the observation outcome is PROBE_FAILURE
    And the monitor becomes UNKNOWN if evidence becomes stale
    And no target DOWN alert is emitted solely for the crash

  @PRD-IAM-005
  Scenario: Sensitive administration is fully audited without secrets
    Given an administrator rotates a secret, changes a role, enrolls a probe and applies a pruned config
    When an owner exports the audit log
    Then each action has actor, resource, request id and time
    And before/after hashes or redacted metadata explain the change
    And no old or new secret value appears

  @PRD-NFR-007
  Scenario: Full supported upgrade reaches 0.3 without data loss
    Given a populated 0.1 release fixture
    When I follow the documented 0.1 to 0.2 to 0.3 upgrade path
    Then resource ids, retained monitoring history and decryptable secrets are preserved
    And organization and project boundaries are valid
    And all 0.3 readiness checks pass
    And a 0.3 backup restores into a fresh 0.3 instance
