# Implementierungsstand am 20. Juli 2026

Baseline: Commit `53be57ab890391c887719a7eaa380cde8e4770d6`. Der Worktree war zu Beginn von `SPEC-010` sauber. Diese Momentaufnahme bewertet Source, Tests, Contracts, 37 Gherkin-Szenarien und vorhandene Evidence; sie ist kein Release-Verdict.

## Zusammenfassung

| Sicht | Anzahl | Aussage |
|---|---:|---|
| Requirements gesamt | 57 | Kanonische IDs aus `specs/00-product-requirements.md` |
| Coverage `full` | 1 | Nur lokaler Ein-Befehl-Start ohne externe Datenbank (`PRD-NFR-001`), noch ohne Release-Evidence |
| Coverage `partial` | 19 | Contract-/Runtime-Grundlagen, Identität, Persistenz und Querschnitts-NFRs |
| Coverage `none` | 37 | Kein entsprechendes Produktverhalten im aktuellen Code |
| Arbeitspakete | 81 | 6 implemented, 73 planned, 2 durch dokumentierte Entscheidungen blockiert |
| Offene Findings | 8 | 5 Spec/Contract/Owner-Themen, 2 Evidence-Lücken und 1 Tooling-Security-Befund; sieben davon high |

Die Zahlen sind bewusst keine Prozent-Fertigstellung. Eine NFR wie „Linux Multi-Arch Releases“ und ein einzelnes Feature hätten sonst dasselbe Gewicht; außerdem sind zusammengesetzte Requirements unterschiedlich groß.

## Vorhandene Implementierung

| Slice | Vorhanden | Noch nicht freigegeben/fehlend |
|---|---|---|
| Repository-Bootstrap | Gepinnte Rust-/Node-/pnpm-Toolchains, Lockfiles, CI, Architektur-/Contract-/Generated-/Security-Gates | Aktueller unabhängiger Clean-Checkout-Verdict fehlt (`EVID-001`) |
| Öffentliche Systemgrenze | `/health/live`, DB-/Migrations-abhängige `/health/ready`, UUIDv7 Request-ID, redigierte Problem Response, Security Header; OpenAPI-Verträge für Browser-Auth/Session/Recovery und sieben prüfungsspezifisch kanonisch abgebildete CheckSpecs | Kein `/api/v1`-Ressourcen- oder Auth-Laufzeitendpunkt, keine AuthN/AuthZ, kein Metrics-Endpunkt, keine Traces; gemeinsame Check-Netzwerkoptionen sind noch offen |
| Web | Strikter React/TypeScript-Build, eingebetteter statischer Shell, semantische Überschrift | Keine Produktnavigation, Async-Zustände, i18n, Monitor-/Status-/Admin-Flows oder vollständige Accessibility |
| Domain/Application | Typisierte UUIDv7-IDs, UTC-Mikrosekunden, Organisation/Projekt/User/Membership/Audit, injizierte Clock/IDs, Argon2id-Port | Kein Monitor, CheckSpec-Domainmodell, Scheduler, Evaluator, Uptime, Outbox oder Permission Engine |
| Persistenz | PostgreSQL-/SQLite-Migration `0001`, sechs Identitätstabellen, gemeinsame Repository-Suite, atomarer lokaler Admin-Bootstrap, append-only Bootstrap-Audit | Keine Sessions/Tokens, Secrets, Monitore/Revisions, Jobs/Observations/Evaluations, Outbox, Statusseiten oder Retention |
| Probe-Vertrag | Proto und generierte Rust-Typen; prüfungsspezifische CheckSpec-Felder, Defaults, Einheiten und Secret-Grenze sind mit OpenAPI/Config abgeglichen | Gemeinsame Proxy-/Resolver-/Adressfamilienoptionen fehlen; kein `takt-probe`, Enrollment, mTLS, Gateway, Offline-Queue, Ingest oder Quorum |
| Akzeptanz | Alle drei Gherkin-Dateien sind syntaktisch valide | Keine der 37 Szenarien ist als ausführbare Produktabnahme gebunden (`EVID-002`) |

Damit ist der zweite Bootstrap-Meilenstein weitgehend implementiert, aber Takt 0.1 noch kein nutzbares Monitoringprodukt. Besonders wichtig: Vorhandene OpenAPI-Schemas sind kein Beleg dafür, dass die Endpunkte laufen; der Router enthält derzeit ausschließlich die zwei Health-Routen.

## Wichtigste gefundene Probleme

`SPEC-001` ist gelöst: Die Daten-IDs sind kanonisch nachverfolgt. `SPEC-013` vereinheitlicht die prüfungsspezifischen CheckSpec-Felder, Einheiten, Defaults, Grenzen und SecretRefs in allen drei Maschinenverträgen mit positiven und negativen Golden-Fixtures; `SPEC-004` bleibt für die in `SPEC-019` isolierten gemeinsamen Netzwerkoptionen offen.

| Finding | Wirkung |
|---|---|
| `SPEC-002` | Monitorabhängigkeiten haben Roadmap und Szenario, aber keine Requirement-ID und keinen Traceability-Eintrag. |
| `SPEC-003` | Der höchstrangige OpenAPI-Vertrag lässt mehrere zwingende 0.1-Verwaltungsressourcen und Operationspfade aus. |
| `SPEC-004` | Die prüfungsspezifischen CheckSpec-Felder sind abgeglichen; gemeinsame Proxy-, Resolver- und Adressfamilienoptionen fehlen bis `SPEC-019`. |
| `SPEC-005` | Der Spec-Index verspricht eine nicht vorhandene `AGENTS.template.md`. |
| `EVID-001` | Historische Evidence enthält keinen aktuellen unabhängigen grünen Verdict für HEAD. |
| `EVID-002` | Gherkin wird nur geparst, nicht ausgeführt; „contracts valid“ darf nicht als Acceptance-Erfolg berichtet werden. |
| `DEC-001` | Lizenz, Name/Paketlage und Security-/Signaturkanäle müssen vor öffentlichem 0.1 bestätigt werden. |
| `SEC-001` | Der transitive Dev-Pfad `openapi-typescript` → `js-yaml@4.2.0` lässt den vollständigen Node-Audit wegen einer High-Severity-DoS-Advisory fehlschlagen; Produktionsabhängigkeiten sind nicht betroffen. |

Details, betroffene Pfade und Resolution stehen in `findings.yaml`.

## Empfohlene nächste Reihenfolge

1. `GOV-002`: den High-Severity-Tooling-Befund `SEC-001` beheben und den vollständigen Node-Audit wieder grün machen.
2. `SPEC-019`: gemeinsame Proxy-, Resolver- und Adressfamilienoptionen in den drei Maschinenverträgen abgleichen und `SPEC-004` schließen.
3. `QA-001`: Acceptance-Szenarien ausführbar zuordnen; anschließend den mittleren Indexfehler `SPEC-005` in `SPEC-014` beheben.
4. `EVID-001` schließen: aktuellen committed Stand unabhängig aus sauberem Checkout validieren; PostgreSQL 16 muss verfügbar sein.
5. `IAM-010` bis `IAM-013`: sichere Session-/Token-Grenze fertigstellen, bevor fachliche Schreibendpunkte entstehen.
6. `MON-010`, `MON-011`, `DATA-010`, `API-010`, `WEB-010`: Monitor-CRUD als erster vollständiger öffentlicher Vertikalschnitt.
7. `CHECK-010` bis `CHECK-012`, `ALERT-010`, `DATA-011`, `WEB-011`: erster echter HTTP-Pfad einschließlich ehrlicher Fehlerklassifikation und atomarer Outbox.
8. Erst danach weitere 0.1-Checktypen, Notifications, deklarative Automation, Statusseiten, vollständige UI und Operations-/Release-Hardening.

Die vollständige Abhängigkeitsfolge bis 0.3 steht in `work-packages.yaml`. Die zwei Pakete `ALERT-030` und `OPS-030` sind bewusst blockiert, bis die referenzierte Spec- beziehungsweise Eigentümerentscheidung vorliegt.

## Während dieser Bestandsaufnahme ausgeführte Prüfungen

| Befehl | Ergebnis |
|---|---|
| `cargo test -p takt-domain -p takt-application -p takt-api -p takt-server -p takt-probe-protocol` | Exit 0; Domain-, Credential-, Health-, Server-, CLI- und Proto-Tests grün |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | Exit 0; sechs SQLite-Migrations-/Repository-/Bootstrap-Fälle grün |
| `pnpm contracts:validate` | Exit 0; OpenAPI/Schema/Proto valide und Gherkin syntaktisch geparst |
| `pnpm check:architecture` | Exit 0 |
| `pnpm check:generated` | Exit 0 |
| `cargo test --workspace --all-features -- --test-threads=1` | Exit 1 im lokalen Debug-Profil: Rust kann beim Bauen von `takt-persistence` die Crate `sqlx` nicht auflösen; nicht als Pass gewertet |
| `cargo test --workspace --all-features --release -- --test-threads=1` | Exit 1 am verpflichtenden PostgreSQL-Contract: `TAKT_TEST_POSTGRES_URL` fehlt; alle zuvor gestarteten Release-Suites waren grün |

Docker/PostgreSQL war lokal nicht verfügbar. Die PostgreSQL-Aussagen bleiben deshalb `evidence_only` aus der bestehenden Evidence und wurden in dieser Bestandsaufnahme nicht als erneut bestanden gewertet. Der vollständige Workspace-Test ist sowohl im Debug- als auch im Release-Profil ausdrücklich **nicht bestanden**, während die separat ausführbaren fokussierten Suites, Clippy und der vollständige Release-Build erfolgreich waren.

## Abschluss von SPEC-010

`SPEC-010` ist `implemented`, nicht `verified`. `pnpm check:tracking` validiert 57 kanonische Requirements ohne Unknown-ID-Ausnahme; Tooltests, Gherkin, SQLite-Vertrag, Readiness-Test, Clippy, Security-/Lizenzgates, Frontend, Playwright und Release-Build sind grün. Der vollständige Workspace-Test endet weiterhin mit Exit 101 ausschließlich am verpflichtenden PostgreSQL-Vertrag, weil lokal weder Docker noch PostgreSQL verfügbar ist. Befehle, Exit Codes und Reviewgrenzen stehen in `docs/implementation-evidence/spec-010-data-requirements.md`.

## Abschluss von SPEC-012

`SPEC-012` ist nach größenbedingter Aufteilung `implemented`, nicht `verified`. Der OpenAPI-Vertrag beschreibt jetzt Browser-Login, Logout, Session, Recovery, Cookie-/CSRF-Grenzen, generische Konto-Fehler, Request-ID und Rate-Limit-Header; die TypeScript-Typen sind regeneriert. Die vier verbleibenden Vertragsfamilien aus `SPEC-003` liegen in den abhängigen Paketen `SPEC-015` bis `SPEC-018`. Laufzeit-Auth bleibt ausdrücklich Folgearbeit ab `IAM-010`; Details stehen in `docs/implementation-evidence/spec-012-auth-contract.md`.

## Abschluss von SPEC-013

`SPEC-013` ist nach Scope-Trennung von den bereits in `MON-011` geplanten Rust-Domänentypen und den in `SPEC-019` isolierten gemeinsamen Netzwerkoptionen `implemented`, nicht `verified`. OpenAPI, Config Schema und Proto bilden jetzt dieselben sieben prüfungsspezifischen Check-Arten einschließlich HTTP-Header/Body/Auth/Assertions, DNS-/ICMP-/TLS-Feldern, Push-GET und Browser-Grenzen ab. Ein Golden-Test prüft sieben gültige und zehn ungültige Fixtures sowie exakte Felder, Defaults und Grenzen; OpenAPI-/Proto-Codegen ist aktuell. Der vollständige Workspace-Test bleibt wegen lokaler Debug-`sqlx`-Auflösung beziehungsweise fehlendem PostgreSQL rot, und der vollständige Node-Audit ist wegen `SEC-001` rot. Details stehen in `docs/implementation-evidence/spec-013-check-spec-contract.md`.
