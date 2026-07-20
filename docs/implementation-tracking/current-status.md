# Implementierungsstand am 20. Juli 2026

Baseline: Commit `f6039b00d374628432aa4af7f2db6d6723af6d00`. Der Worktree war zu Beginn der Bestandsaufnahme sauber. Diese Momentaufnahme bewertet Source, Tests, Contracts, 37 Gherkin-Szenarien und vorhandene Evidence; sie ist kein Release-Verdict.

## Zusammenfassung

| Sicht | Anzahl | Aussage |
|---|---:|---|
| Requirements gesamt | 54 | Kanonische IDs aus `specs/00-product-requirements.md` |
| Coverage `full` | 1 | Nur lokaler Ein-Befehl-Start ohne externe Datenbank (`PRD-NFR-001`), noch ohne Release-Evidence |
| Coverage `partial` | 15 | Contract-/Runtime-Grundlagen, Identität, Persistenz und Querschnitts-NFRs |
| Coverage `none` | 38 | Kein entsprechendes Produktverhalten im aktuellen Code |
| Arbeitspakete | 75 | 3 implemented, 70 planned, 2 durch dokumentierte Entscheidungen blockiert |
| Offene Findings | 8 | 6 Spec/Contract/Owner-Themen und 2 Evidence-Lücken; sieben davon high |

Die Zahlen sind bewusst keine Prozent-Fertigstellung. Eine NFR wie „Linux Multi-Arch Releases“ und ein einzelnes Feature hätten sonst dasselbe Gewicht; außerdem sind zusammengesetzte Requirements unterschiedlich groß.

## Vorhandene Implementierung

| Slice | Vorhanden | Noch nicht freigegeben/fehlend |
|---|---|---|
| Repository-Bootstrap | Gepinnte Rust-/Node-/pnpm-Toolchains, Lockfiles, CI, Architektur-/Contract-/Generated-/Security-Gates | Aktueller unabhängiger Clean-Checkout-Verdict fehlt (`EVID-001`) |
| Öffentliche Systemgrenze | `/health/live`, DB-/Migrations-abhängige `/health/ready`, UUIDv7 Request-ID, redigierte Problem Response, Security Header | Kein `/api/v1`-Ressourcenendpunkt, keine AuthN/AuthZ, kein Metrics-Endpunkt, keine Traces |
| Web | Strikter React/TypeScript-Build, eingebetteter statischer Shell, semantische Überschrift | Keine Produktnavigation, Async-Zustände, i18n, Monitor-/Status-/Admin-Flows oder vollständige Accessibility |
| Domain/Application | Typisierte UUIDv7-IDs, UTC-Mikrosekunden, Organisation/Projekt/User/Membership/Audit, injizierte Clock/IDs, Argon2id-Port | Kein Monitor, CheckSpec-Domainmodell, Scheduler, Evaluator, Uptime, Outbox oder Permission Engine |
| Persistenz | PostgreSQL-/SQLite-Migration `0001`, sechs Identitätstabellen, gemeinsame Repository-Suite, atomarer lokaler Admin-Bootstrap, append-only Bootstrap-Audit | Keine Sessions/Tokens, Secrets, Monitore/Revisions, Jobs/Observations/Evaluations, Outbox, Statusseiten oder Retention |
| Probe-Vertrag | Proto und generierte Rust-Typen | Kein `takt-probe`, Enrollment, mTLS, Gateway, Offline-Queue, Ingest oder Quorum |
| Akzeptanz | Alle drei Gherkin-Dateien sind syntaktisch valide | Keine der 37 Szenarien ist als ausführbare Produktabnahme gebunden (`EVID-002`) |

Damit ist der zweite Bootstrap-Meilenstein weitgehend implementiert, aber Takt 0.1 noch kein nutzbares Monitoringprodukt. Besonders wichtig: Vorhandene OpenAPI-Schemas sind kein Beleg dafür, dass die Endpunkte laufen; der Router enthält derzeit ausschließlich die zwei Health-Routen.

## Wichtigste gefundene Probleme

| Finding | Wirkung |
|---|---|
| `SPEC-001` | Drei in Migrationen/Tests/Evidence verwendete `PRD-DATA-*`-IDs existieren nicht in der kanonischen Spec. |
| `SPEC-002` | Monitorabhängigkeiten haben Roadmap und Szenario, aber keine Requirement-ID und keinen Traceability-Eintrag. |
| `SPEC-003` | Der höchstrangige OpenAPI-Vertrag lässt mehrere zwingende 0.1-Verwaltungsressourcen und Operationspfade aus. |
| `SPEC-004` | OpenAPI, Config Schema, Proto und Kapitel 04 widersprechen sich beim CheckSpec-Umfang. |
| `SPEC-005` | Der Spec-Index verspricht eine nicht vorhandene `AGENTS.template.md`. |
| `EVID-001` | Historische Evidence enthält keinen aktuellen unabhängigen grünen Verdict für HEAD. |
| `EVID-002` | Gherkin wird nur geparst, nicht ausgeführt; „contracts valid“ darf nicht als Acceptance-Erfolg berichtet werden. |
| `DEC-001` | Lizenz, Name/Paketlage und Security-/Signaturkanäle müssen vor öffentlichem 0.1 bestätigt werden. |

Details, betroffene Pfade und Resolution stehen in `findings.yaml`.

## Empfohlene nächste Reihenfolge

1. `SPEC-010`, `SPEC-012`, `SPEC-013` und `QA-001`: Requirement-IDs bereinigen, fehlende Verträge featureweise schließen, CheckSpec vereinheitlichen und Acceptance ausführbar zuordnen.
2. `EVID-001` schließen: aktuellen committed Stand unabhängig aus sauberem Checkout validieren; PostgreSQL 16 muss verfügbar sein.
3. `IAM-010` bis `IAM-013`: sichere Session-/Token-Grenze fertigstellen, bevor fachliche Schreibendpunkte entstehen.
4. `MON-010`, `MON-011`, `DATA-010`, `API-010`, `WEB-010`: Monitor-CRUD als erster vollständiger öffentlicher Vertikalschnitt.
5. `CHECK-010` bis `CHECK-012`, `ALERT-010`, `DATA-011`, `WEB-011`: erster echter HTTP-Pfad einschließlich ehrlicher Fehlerklassifikation und atomarer Outbox.
6. Erst danach weitere 0.1-Checktypen, Notifications, deklarative Automation, Statusseiten, vollständige UI und Operations-/Release-Hardening.

Die vollständige Abhängigkeitsfolge bis 0.3 steht in `work-packages.yaml`. Die zwei Pakete `ALERT-030` und `OPS-030` sind bewusst blockiert, bis die referenzierte Spec- beziehungsweise Eigentümerentscheidung vorliegt.

## Während dieser Bestandsaufnahme ausgeführte Prüfungen

| Befehl | Ergebnis |
|---|---|
| `cargo test -p takt-domain -p takt-application -p takt-api -p takt-server -p takt-probe-protocol` | Exit 0; Domain-, Credential-, Health-, Server-, CLI- und Proto-Tests grün |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | Exit 0; sechs SQLite-Migrations-/Repository-/Bootstrap-Fälle grün |
| `pnpm contracts:validate` | Exit 0; OpenAPI/Schema/Proto valide und Gherkin syntaktisch geparst |
| `pnpm check:architecture` | Exit 0 |
| `pnpm check:generated` | Exit 0 |
| `cargo test --workspace --all-features -- --test-threads=1` | Exit 1 ausschließlich am verpflichtenden PostgreSQL-Contract: `TAKT_TEST_POSTGRES_URL` fehlt; alle zuvor gestarteten Suites waren grün |

Docker/PostgreSQL war lokal nicht verfügbar. Die PostgreSQL-Aussagen bleiben deshalb `evidence_only` aus der bestehenden Evidence und wurden in dieser Bestandsaufnahme nicht als erneut bestanden gewertet. Der vollständige Workspace-Test ist damit ausdrücklich **nicht bestanden**, während alle separat ausführbaren Gates erfolgreich waren.
