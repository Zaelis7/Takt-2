# Implementierungsstand am 21. Juli 2026

Baseline: Commit `5cb6dace3ec75241205b6ad0ed8b9d133490e8e8`. Der Worktree war zu Beginn von `IAM-010` sauber. Diese Momentaufnahme bewertet Source, Tests, Contracts, 37 Gherkin-Szenarien und vorhandene Evidence; sie ist kein Release-Verdict.

## Zusammenfassung

| Sicht | Anzahl | Aussage |
|---|---:|---|
| Requirements gesamt | 57 | Kanonische IDs aus `specs/00-product-requirements.md` |
| Coverage `full` | 1 | Nur lokaler Ein-Befehl-Start ohne externe Datenbank (`PRD-NFR-001`), noch ohne Release-Evidence |
| Coverage `partial` | 19 | Contract-/Runtime-Grundlagen, Identität, Persistenz und Querschnitts-NFRs |
| Coverage `none` | 37 | Kein entsprechendes Produktverhalten im aktuellen Code |
| Arbeitspakete | 82 | 12 implemented, 68 planned, 2 durch dokumentierte Entscheidungen blockiert |
| Offene Findings | 5 | 3 Spec/Contract/Owner-Themen und 2 Evidence-Lücken; alle fünf high |

Die Zahlen sind bewusst keine Prozent-Fertigstellung. Eine NFR wie „Linux Multi-Arch Releases“ und ein einzelnes Feature hätten sonst dasselbe Gewicht; außerdem sind zusammengesetzte Requirements unterschiedlich groß.

## Vorhandene Implementierung

| Slice | Vorhanden | Noch nicht freigegeben/fehlend |
|---|---|---|
| Repository-Bootstrap | Gepinnte Rust-/Node-/pnpm-Toolchains, Lockfiles, CI, Architektur-/Contract-/Generated-/Security-Gates, sicherer `js-yaml`-Codegen-Pfad sowie verpflichtender Größen-/Validierungs-Preflight für aktive Arbeitspakete | Aktueller unabhängiger Clean-Checkout-Verdict fehlt (`EVID-001`); Schätzwerte werden noch nicht mit tatsächlichem Diff und Laufzeit abgeglichen |
| Öffentliche Systemgrenze | `/health/live`, DB-/Migrations-abhängige `/health/ready`, UUIDv7 Request-ID, redigierte Problem Response, Security Header; OpenAPI-Verträge für Browser-Auth/Session/Recovery mit stabilen Auth-Problem-Codes und sieben vollständig kanonisch abgebildete CheckSpecs einschließlich gemeinsamer Netzwerkoptionen | Kein `/api/v1`-Ressourcen- oder Auth-Laufzeitendpunkt, keine AuthN/AuthZ, kein Metrics-Endpunkt, keine Traces |
| Web | Strikter React/TypeScript-Build, eingebetteter statischer Shell, semantische Überschrift | Keine Produktnavigation, Async-Zustände, i18n, Monitor-/Status-/Admin-Flows oder vollständige Accessibility |
| Domain/Application | Typisierte UUIDv7-IDs, UTC-Mikrosekunden, Organisation/Projekt/User/Membership/Audit, injizierte Clock/IDs, Argon2id-Port sowie deterministische Session-Laufzeit-, Rotation-, Revoke- und CSRF-Regeln | Keine Session-/Recovery-Orchestrierung; kein Monitor, CheckSpec-Domainmodell, Scheduler, Evaluator, Uptime, Outbox oder Permission Engine |
| Persistenz | PostgreSQL-/SQLite-Migration `0001`, sechs Identitätstabellen, gemeinsame Repository-Suite, atomarer lokaler Admin-Bootstrap, append-only Bootstrap-Audit | Keine Sessions/Tokens, Secrets, Monitore/Revisions, Jobs/Observations/Evaluations, Outbox, Statusseiten oder Retention |
| Probe-Vertrag | Proto und generierte Rust-Typen; prüfungsspezifische sowie gemeinsame Proxy-/Resolver-/Adressfamilienoptionen, Defaults, Einheiten und Secret-Grenzen sind mit OpenAPI/Config abgeglichen | Kein `takt-probe`, Enrollment, mTLS, Gateway, Offline-Queue, Ingest oder Quorum |
| Akzeptanz | Alle drei Gherkin-Dateien sind syntaktisch valide; alle 37 Szenarien besitzen ein maschinengeprüftes Manifest-Binding zu Requirements und Umsetzungspaketen | Alle 37 Bindings sind noch `planned` und besitzen kein Verhaltens-Testkommando; der Release-Runner schlägt deshalb ehrlich fehl (`EVID-002`) |

Damit ist der zweite Bootstrap-Meilenstein weitgehend implementiert, aber Takt 0.1 noch kein nutzbares Monitoringprodukt. Besonders wichtig: Vorhandene OpenAPI-Schemas sind kein Beleg dafür, dass die Endpunkte laufen; der Router enthält derzeit ausschließlich die zwei Health-Routen.

## Wichtigste gefundene Probleme

`SPEC-001` ist gelöst: Die Daten-IDs sind kanonisch nachverfolgt. `SPEC-013` vereinheitlicht die prüfungsspezifischen CheckSpec-Felder; `SPEC-019` ergänzt die gemeinsamen Proxy-, Resolver- und Adressfamilienoptionen mit denselben Namen, Grenzen und SecretRefs in allen drei Maschinenverträgen. Damit ist `SPEC-004` vollständig gelöst. `SPEC-005` ist durch Entfernen des historisch unbelegten Template-Eintrags und einen CI-geprüften Spec-Index gelöst. `SEC-001` ist durch die gepinnte `js-yaml@4.3.0`-Auflösung, einen Lockfile-Regressionsfall und den grünen vollständigen Node-Audit gelöst.

| Finding | Wirkung |
|---|---|
| `SPEC-002` | Monitorabhängigkeiten haben Roadmap und Szenario, aber keine Requirement-ID und keinen Traceability-Eintrag. |
| `SPEC-003` | Der höchstrangige OpenAPI-Vertrag lässt mehrere zwingende 0.1-Verwaltungsressourcen und Operationspfade aus. |
| `EVID-001` | Historische Evidence enthält keinen aktuellen unabhängigen grünen Verdict für HEAD. |
| `EVID-002` | Syntax und 37-entry Binding-Inventar sind geprüft, aber noch kein Produkt-Szenario ist runnable; „contracts/bindings valid“ darf nicht als Acceptance-Erfolg berichtet werden. |
| `DEC-001` | Lizenz, Name/Paketlage und Security-/Signaturkanäle müssen vor öffentlichem 0.1 bestätigt werden. |

Details, betroffene Pfade und Resolution stehen in `findings.yaml`.

## Empfohlene nächste Reihenfolge

1. `EVID-001` schließen: aktuellen committed Stand unabhängig aus sauberem Checkout validieren; PostgreSQL 16 ist lokal verfügbar, aber der IAM-010-Change ist noch uncommitted.
2. `IAM-011` bis `IAM-013`: sichere Session-/Token-Grenze fertigstellen, bevor fachliche Schreibendpunkte entstehen.
3. `MON-010`, `MON-011`, `DATA-010`, `API-010`, `WEB-010`: Monitor-CRUD als erster vollständiger öffentlicher Vertikalschnitt.
4. `CHECK-010` bis `CHECK-012`, `ALERT-010`, `DATA-011`, `WEB-011`: erster echter HTTP-Pfad einschließlich ehrlicher Fehlerklassifikation und atomarer Outbox.
5. Erst danach weitere 0.1-Checktypen, Notifications, deklarative Automation, Statusseiten, vollständige UI und Operations-/Release-Hardening.

Die vollständige Abhängigkeitsfolge bis 0.3 steht in `work-packages.yaml`. Die zwei Pakete `ALERT-030` und `OPS-030` sind bewusst blockiert, bis die referenzierte Spec- beziehungsweise Eigentümerentscheidung vorliegt.

## Während dieser Bestandsaufnahme ausgeführte Prüfungen

| Befehl | Ergebnis |
|---|---|
| `cargo test -p takt-domain -p takt-application -p takt-api -p takt-server -p takt-probe-protocol` | Exit 0; Domain-, Credential-, Health-, Server-, CLI- und Proto-Tests grün |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | Exit 0; sechs SQLite-Migrations-/Repository-/Bootstrap-Fälle grün |
| `pnpm contracts:validate` | Exit 0; OpenAPI/Schema/Proto valide und Gherkin syntaktisch geparst |
| `pnpm check:architecture` | Exit 0 |
| `pnpm check:generated` | Exit 0 |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | Exit 0; vollständiger Workspace einschließlich echter PostgreSQL-16.9- und SQLite-Verträge grün |
| `cargo build --workspace --all-features --release --locked` | Exit 0; vollständiger optimierter Workspace-Build grün |

Docker Desktop wurde für die Validierung gestartet. Die Repository-Suite lief gegen das gepinnte Image `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` auf Loopback und bestand ohne Skip. Das ist vollständige lokale Working-Tree-Validation, aber noch keine unabhängige, commit-gebundene oder CI-basierte Freigabe; `EVID-001` bleibt deshalb offen.

## Abschluss von SPEC-010

`SPEC-010` ist `implemented`, nicht `verified`. `pnpm check:tracking` validiert 57 kanonische Requirements ohne Unknown-ID-Ausnahme; Tooltests, Gherkin, SQLite-Vertrag, Readiness-Test, Clippy, Security-/Lizenzgates, Frontend, Playwright und Release-Build sind grün. Der vollständige Workspace-Test endet weiterhin mit Exit 101 ausschließlich am verpflichtenden PostgreSQL-Vertrag, weil lokal weder Docker noch PostgreSQL verfügbar ist. Befehle, Exit Codes und Reviewgrenzen stehen in `docs/implementation-evidence/spec-010-data-requirements.md`.

## Abschluss von SPEC-012

`SPEC-012` ist nach größenbedingter Aufteilung `implemented`, nicht `verified`. Der OpenAPI-Vertrag beschreibt jetzt Browser-Login, Logout, Session, Recovery, Cookie-/CSRF-Grenzen, generische Konto-Fehler, Request-ID und Rate-Limit-Header; die TypeScript-Typen sind regeneriert. Die vier verbleibenden Vertragsfamilien aus `SPEC-003` liegen in den abhängigen Paketen `SPEC-015` bis `SPEC-018`. Laufzeit-Auth bleibt ausdrücklich Folgearbeit ab `IAM-010`; Details stehen in `docs/implementation-evidence/spec-012-auth-contract.md`.

## Abschluss von SPEC-013

`SPEC-013` ist nach Scope-Trennung von den bereits in `MON-011` geplanten Rust-Domänentypen und den damals in `SPEC-019` isolierten gemeinsamen Netzwerkoptionen `implemented`, nicht `verified`. OpenAPI, Config Schema und Proto bilden dieselben sieben prüfungsspezifischen Check-Arten einschließlich HTTP-Header/Body/Auth/Assertions, DNS-/ICMP-/TLS-Feldern, Push-GET und Browser-Grenzen ab. `SPEC-019` hat die verschobenen Netzwerkoptionen inzwischen ergänzt. Der vollständige Workspace-Test bleibt wegen fehlendem PostgreSQL rot; der damals rote Node-Audit ist durch `GOV-002` behoben. Details stehen in `docs/implementation-evidence/spec-013-check-spec-contract.md`.

## Abschluss von GOV-003

`GOV-003` ist `implemented`, nicht `verified`. Jedes auf `in_progress` gesetzte Paket benötigt nun Scope, Ausschlüsse, betroffene Artefakte sowie eine Zeilen- und Validierungszeitschätzung. `pnpm check:tracking` warnt ab 600 handgeschriebenen Zeilen und lehnt Pakete über 800 Zeilen oder 30 Validierungsminuten ab. Bestehende geplante und abgeschlossene Pakete bleiben ohne rückwirkende Schätzpflicht gültig; Details stehen in `docs/implementation-evidence/gov-003-package-preflight.md`.

## Abschluss von GOV-002

`GOV-002` ist `implemented`, nicht `verified`. Der pnpm-11-Workspace erzwingt jetzt `js-yaml@4.3.0` für den transitiven OpenAPI-Codegen-Pfad; ein `PRD-API-002`-Regressionsfall lehnt jede ältere Lockfile-Auflösung ab. Der vollständige Node-Audit, Contract-, Generated-Drift- und Lizenz-Gates sind grün, womit `SEC-001` gelöst ist. Der vollständige Rust-Workspace-Test bleibt mangels laufender PostgreSQL-Instanz nicht bestanden; Details stehen in `docs/implementation-evidence/gov-002-node-supply-chain.md`.

## Abschluss von SPEC-019

`SPEC-019` ist `implemented`, nicht `verified`. OpenAPI, deklaratives Config Schema und Proto besitzen jetzt dieselben Resolver- und Adressfamilienoptionen für HTTP, TCP, DNS, ICMP, TLS und Browser sowie dieselbe Proxy-Grenze für HTTP, TCP, TLS und Browser. Authority-only URIs, verbotene eingebettete Credentials, reine SecretRef-Proxy-Auth, Push-Ausschluss und additive Proto-Tags sind positiv und negativ getestet; generierte TypeScript-/Rust-Typen sind aktuell. `SPEC-004` ist damit gelöst. Der vollständige Rust-Workspace-Test bleibt mangels PostgreSQL nicht bestanden; Details stehen in `docs/implementation-evidence/spec-019-network-options-contract.md`.

## Abschluss von QA-001

`QA-001` ist `implemented`, nicht `verified`. `specs/acceptance/bindings.yaml` ordnet alle 37 Szenariodefinitionen exakt ihren Gherkin-Dateien, PRD-Tags und verantwortlichen Umsetzungspaketen zu. `pnpm acceptance:check` prüft dieses Inventar separat von der bestehenden Syntaxvalidierung; ein fehlendes Binding, Tag-Drift, unbekanntes Paket oder ein als runnable markierter Eintrag ohne Testkommando ist rot. `pnpm acceptance:run -- --release v0.1` bleibt bewusst rot, solange eines der 15 v0.1-Bindings geplant ist. Deshalb bleibt `EVID-002` offen und die Requirement-Coverage unverändert. Der vollständige Rust-Workspace-Test bleibt mangels PostgreSQL nicht bestanden; Details stehen in `docs/implementation-evidence/qa-001-acceptance-bindings.md`.

## Abschluss von SPEC-014

`SPEC-014` ist `implemented`, nicht `verified`. Der nie vorhandene `AGENTS.template.md`-Eintrag wurde aus `specs/README.md` entfernt; Kapitel 09 und das echte Root-`AGENTS.md` bleiben unverändert. `pnpm check:spec-index` validiert alle 16 verbliebenen Literal- und Globpfade und ist in CI verdrahtet. Positive Literal-/Glob-Fälle sowie fehlende, nicht treffende und aus dem Spec-Paket ausbrechende Pfade sind getestet. `SPEC-005` ist gelöst; keine Requirement-Coverage wurde verändert. Der vollständige Rust-Workspace-Test bleibt mangels PostgreSQL nicht bestanden; Details stehen in `docs/implementation-evidence/spec-014-index-integrity.md`.

## Abschluss von IAM-010

`IAM-010` ist `implemented`, nicht `verified`. Die frameworkfreie Domain bildet die standardmäßigen 12 Stunden Inaktivität und sieben Tage absolute Laufzeit, konfigurierbare validierte Grenzen, Aktivitätsverlängerung ohne Überschreiten des absoluten Ablaufs, Rotation nach Login/Rechteänderung/sensibler Aktion, Revoke bei Logout/Recovery sowie sessiongebundene CSRF-Entscheidungen deterministisch ab. Der OpenAPI-Vertrag verwendet für Auth-Fehler jetzt stabile, generische Codes und unterscheidet Recovery-Tokenfehler nicht nach Existenz, Ablauf oder Einmalverwendung. Sämtliche lokalen Gates einschließlich echtem PostgreSQL 16.9, SQLite, Supply Chain, Frontend, Playwright und Release-Build sind grün. Persistenz und Laufzeit folgen in `IAM-011`/`IAM-012`; unabhängige commit-gebundene Review-/CI-Evidence fehlt weiterhin. Details stehen in `docs/implementation-evidence/iam-010-auth-domain-contract.md`.
