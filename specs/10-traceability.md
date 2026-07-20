# 10 – Nachverfolgbarkeit

## 1. Zweck

Jede Muss-Anforderung muss in Code, Tests und Release Evidence nachverfolgbar sein. Testnamen, Issues und Pull Requests verwenden die jeweilige Requirement-ID. Eine ID darf nicht als „erfüllt“ markiert werden, solange nur ein Teil ihres Releaseumfangs existiert.

## 2. Produktanforderungen

| IDs | Zielrelease | Primärer Vertrag | Abnahme |
|---|---:|---|---|
| PRD-MON-001, PRD-MON-003, PRD-MON-004, PRD-MON-005, PRD-MON-006 | 0.1 | OpenAPI, Config Schema | `v0.1.feature` |
| PRD-MON-002 | 0.1 | OpenAPI CheckSpec | `v0.1.feature` Check Outline |
| PRD-MON-007 | 0.2 | `probe.proto` | `v0.2.feature` Probe/Quorum |
| PRD-MON-008 | 0.3 | OpenAPI/Proto BrowserCheck | `v0.3.feature` Browser |
| PRD-API-001, PRD-API-002, PRD-API-003, PRD-API-004, PRD-API-005 | 0.1 | `openapi.yaml` | `v0.1.feature` API/Idempotenz |
| PRD-AUT-001, PRD-AUT-002, PRD-AUT-003 | 0.1 | Config Schema/OpenAPI | `v0.1.feature` Apply/Prune |
| PRD-AUT-004 | 0.3 | OpenAPI v1 | `v0.3.feature` Terraform |
| PRD-AUT-005 | 0.3 | Config Schema/OpenAPI | `v0.3.feature` Discovery |
| PRD-ALT-001, PRD-ALT-002, PRD-ALT-003, PRD-ALT-004, PRD-ALT-006 | 0.1 | Domain/OpenAPI | `v0.1.feature` Threshold/Internal Error |
| PRD-ALT-005 | 0.2 | OpenAPI-Erweiterung vor Umsetzung | `v0.2.feature` Maintenance/Flapping |
| PRD-NOT-001, PRD-NOT-002, PRD-NOT-003, PRD-NOT-004 | 0.1 | OpenAPI-Erweiterung vor Umsetzung | `v0.1.feature` Notification |
| PRD-NOT-005 | nach 0.3 | künftiger Pluginvertrag | nicht Release-relevant |
| PRD-STA-001, PRD-STA-002, PRD-STA-006 | 0.1 | OpenAPI Public Projection | `v0.1.feature` Status/Uptime |
| PRD-STA-003, PRD-STA-004 | 0.2 | OpenAPI-Erweiterung vor Umsetzung | `v0.2.feature` Incident |
| PRD-STA-005 | 0.3 | OpenAPI-Erweiterung vor Umsetzung | `v0.3.feature` Subscriber |
| PRD-IAM-001 | 0.1 | OpenAPI Security Schemes | `v0.1.feature` Token Scope |
| PRD-IAM-002, PRD-IAM-003, PRD-IAM-004, PRD-IAM-005 | 0.3 | OpenAPI-Erweiterung vor Umsetzung | `v0.3.feature` IAM/Audit |
| PRD-DATA-001, PRD-DATA-002, PRD-DATA-004 | 0.1 Basis, fortlaufend | Repository- und Migrationsverträge | `v0.1.feature` Persistenzparität/Migration |
| PRD-MIG-001, PRD-MIG-002, PRD-MIG-003, PRD-MIG-004 | 0.2 | Import Plan Contract vor Umsetzung | `v0.2.feature` Import |
| PRD-NFR-001, PRD-NFR-002, PRD-NFR-003, PRD-NFR-004, PRD-NFR-005, PRD-NFR-006, PRD-NFR-007, PRD-NFR-008, PRD-NFR-009, PRD-NFR-010 | 0.1 Basis, fortlaufend | Build/Operations | alle Suites + Evidence |

„OpenAPI-Erweiterung vor Umsetzung“ ist keine Lücke mit Implementierungsfreiheit: Das Feature darf erst begonnen werden, wenn die im Fachkapitel geforderte Ressource als konkreter Contract Change reviewt wurde.

## 3. Architektur-Invarianten

| Invariante | Automatischer Nachweis |
|---|---|
| Domain importiert keine I/O-Frameworks | Dependency-/Architecture-Test |
| interner Fehler ist kein Target-Ausfall | Property-, Integration- und Chaos-Test |
| Evaluation, Transition und Outbox atomar | DB-Integrationstest mit Failpoints |
| Observation-Ingest idempotent | Duplicate-/Reordering-Test |
| UI nutzt nur öffentliche API | E2E plus Build ohne private Endpunkte |
| Secrets verlassen Redaction-Grenze nicht | Golden/Property Tests und Secret Scan |
| Cross-Tenant-Zugriff unmöglich | generierte negative Permission-Matrix |
| SQLite/PostgreSQL fachlich gleich | gemeinsamer Repository Contract Test |
| deklaratives Apply ist idempotent | Plan/Apply/Plan Acceptance |
| öffentliche Projektion ist redigiert | Contract- und E2E-Test |

## 4. Release-Evidence-Matrix

| Evidence | 0.1 | 0.2 | 0.3 |
|---|:---:|:---:|:---:|
| Rust/Frontend Static Gates | ✓ | ✓ | ✓ |
| OpenAPI/Schema/Proto Validation | ✓ | ✓ | ✓ |
| PostgreSQL/SQLite Contract Tests | ✓ | ✓ | ✓ |
| Migration + Backup/Restore | ✓ | ✓ | ✓ |
| Browser E2E + Accessibility | ✓ | ✓ | ✓ |
| Last- und Soak-Test | ✓ | ✓ | ✓ |
| Remote-Probe Disconnect/Queue | – | ✓ | ✓ |
| Uptime-Kuma Import-Fixture | – | ✓ | ✓ |
| Cross-Tenant-/OIDC-Matrix | – | – | ✓ |
| Terraform/Discovery Convergence | – | – | ✓ |
| Browserworker-Isolation | – | – | ✓ |
| SBOM/Signatur | empfohlen | empfohlen | ✓ |

## 5. Mindestmetadaten im Code

- Migrationsdateien nennen zugehöriges Issue im Header-Kommentar.
- Contract-Tests nennen die Operation-ID und Requirement-ID.
- Gherkin-Szenarien behalten ihre `@PRD-*` Tags.
- Pull-Request-Beschreibung enthält `Requirements:` und `Acceptance:`.
- Release Evidence enthält eine automatisch erzeugte Liste aller für das Release erwarteten IDs und deren Tests.

## 6. Umgang mit Lücken

Findet ein Agent eine Anforderung ohne eindeutigen Vertrag oder Abnahmepfad, eröffnet er zuerst eine kleine Spec-Änderung. Er implementiert nicht „nach bestem Gefühl“ parallel. Ein zusätzlicher Test darf die Spec präzisieren; eine neue öffentliche Semantik erfordert eine versionierte Spec-Entscheidung.
