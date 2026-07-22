# Implementierungsstand am 22. Juli 2026

Baseline: Commit `4ef411a4718d21fc4f364494dc3810f716215e98`. Der unabhängige Validator-Checkout war zu Beginn von `EVID-001` sauber. Diese Momentaufnahme bewertet Source, Tests, Contracts, 37 Gherkin-Szenarien und vorhandene Evidence; sie ist kein Release-Verdict.

## Zusammenfassung

| Sicht | Anzahl | Aussage |
|---|---:|---|
| Requirements gesamt | 57 | Kanonische IDs aus `specs/00-product-requirements.md` |
| Coverage `full` | 1 | Nur lokaler Ein-Befehl-Start ohne externe Datenbank (`PRD-NFR-001`), noch ohne Release-Evidence |
| Coverage `partial` | 22 | Contract-/Runtime-Grundlagen, Identität, Persistenz und Querschnitts-NFRs |
| Coverage `none` | 34 | Kein entsprechendes Produktverhalten im aktuellen Code |
| Arbeitspakete | 95 | 26 implemented, 67 planned, 0 in progress, 2 durch dokumentierte Entscheidungen blockiert |
| Offene Findings | 4 | 3 Spec/Contract/Owner-Themen und 1 Evidence-Lücke; alle vier high |

Die Zahlen sind bewusst keine Prozent-Fertigstellung. Eine NFR wie „Linux Multi-Arch Releases“ und ein einzelnes Feature hätten sonst dasselbe Gewicht; außerdem sind zusammengesetzte Requirements unterschiedlich groß.

## Vorhandene Implementierung

| Slice | Vorhanden | Noch nicht freigegeben/fehlend |
|---|---|---|
| Repository-Bootstrap | Gepinnte Rust-/Node-/pnpm-Toolchains, Lockfiles, CI, Architektur-/Contract-/Generated-/Security-Gates, abgesicherte `js-yaml`-/`fast-uri`-Auflösungen sowie verpflichtender Größen-/Validierungs-Preflight für aktive Arbeitspakete | Commit-gebundener unabhängiger Clean-Checkout-Verdict liegt für `4ef411a` vor; CI- und Release-Evidence sowie automatischer Soll-/Ist-Abgleich der Paketschätzungen fehlen |
| Öffentliche Systemgrenze | Health und komponierte Login-/Session-/Logout-Routen mit sicheren Cookies, CSRF und festem 10/Minute-IP-/Kontolimit; API-Token-CRUD ist mit One-time-Ausgabe, Scopes, Filtern, Idempotenz und ETag vertraglich definiert | API-Token- und Recovery-Runtime, weitere `/api/v1`-Ressourcen, allgemeine AuthZ, Metrics und Traces fehlen |
| Web | Strikter React/TypeScript-Build, eingebetteter statischer Shell, semantische Überschrift | Keine Produktnavigation, Async-Zustände, i18n, Monitor-/Status-/Admin-Flows oder vollständige Accessibility |
| Domain/Application | Browser-Login, Session-Aktivität/CSRF-Rotation und Logout orchestrieren Credential-Prüfung, 256-Bit-Werte, Digestgrenzen, Kontext und Audit frameworkfrei und sind mit HTTP/Persistenz komponiert; API-Token besitzen redigierte 256-Bit-Secret-/Argon2id-Grenzen, kanonische CIDRs, exakte Scope-Entscheidungen und eine an Actor/Methode/Pfad/Key/Hash gebundene AEAD-Replay-Grenze | Patch-/Revoke-Idempotenz, API-Token-Use-Cases und HTTP fehlen; keine Recovery-HTTP-Orchestrierung, Monitore, Scheduler, Evaluator, Uptime, Outbox oder allgemeine Permission Engine |
| Persistenz | PostgreSQL-/SQLite-Migrationen `0001` bis `0005`, Identitäts-, Session-, Recovery- und API-Token-Repositories sowie atomarer verschlüsselter Create-Replay mit Konkurrenz-, Konflikt-, Rollback-, Ablauf- und Cleanup-Parität; Token-Lifecycle ist optimistisch versioniert und Last-used monoton | Patch-/Revoke-Idempotenz, Secret Store, Monitor/Revision, Job/Observation/Evaluation, Outbox, Statusseite und Retention fehlen |
| Probe-Vertrag | Proto und generierte Rust-Typen; prüfungsspezifische sowie gemeinsame Proxy-/Resolver-/Adressfamilienoptionen, Defaults, Einheiten und Secret-Grenzen sind mit OpenAPI/Config abgeglichen | Kein `takt-probe`, Enrollment, mTLS, Gateway, Offline-Queue, Ingest oder Quorum |
| Akzeptanz | Alle drei Gherkin-Dateien sind syntaktisch valide; alle 37 Szenarien besitzen ein maschinengeprüftes Manifest-Binding zu Requirements und Umsetzungspaketen | Alle 37 Bindings sind noch `planned` und besitzen kein Verhaltens-Testkommando; der Release-Runner schlägt deshalb ehrlich fehl (`EVID-002`) |

Damit ist der zweite Bootstrap-Meilenstein weitgehend implementiert, aber Takt 0.1 noch kein nutzbares Monitoringprodukt. Login, Session und Logout laufen; für andere OpenAPI-Schemas fehlt weiterhin Runtime-Evidence.

## Wichtigste gefundene Probleme

`SPEC-001` ist gelöst: Die Daten-IDs sind kanonisch nachverfolgt. `SPEC-013` vereinheitlicht die prüfungsspezifischen CheckSpec-Felder; `SPEC-019` ergänzt die gemeinsamen Proxy-, Resolver- und Adressfamilienoptionen mit denselben Namen, Grenzen und SecretRefs in allen drei Maschinenverträgen. Damit ist `SPEC-004` vollständig gelöst. `SPEC-005` ist durch Entfernen des historisch unbelegten Template-Eintrags und einen CI-geprüften Spec-Index gelöst. `SPEC-006` ist durch den eindeutigen, verschlüsselten und auf 24 Stunden begrenzten API-Token-Create-Replay-Vertrag gelöst. `SEC-001` und `SEC-002` sind durch gepinnte sichere `js-yaml`-/`fast-uri`-Auflösungen, Lockfile-Regressionsfälle und den grünen vollständigen Node-Audit gelöst. `EVID-001` ist durch den unabhängigen Clean-Checkout-Verdict für Commit `4ef411a` gelöst, ohne den historischen fehlgeschlagenen Bootstrap-Verdict zu überschreiben.

| Finding | Wirkung |
|---|---|
| `SPEC-002` | Monitorabhängigkeiten haben Roadmap und Szenario, aber keine Requirement-ID und keinen Traceability-Eintrag. |
| `SPEC-003` | Der höchstrangige OpenAPI-Vertrag lässt mehrere zwingende 0.1-Verwaltungsressourcen und Operationspfade aus. |
| `EVID-002` | Syntax und 37-entry Binding-Inventar sind geprüft, aber noch kein Produkt-Szenario ist runnable; „contracts/bindings valid“ darf nicht als Acceptance-Erfolg berichtet werden. |
| `DEC-001` | Lizenz, Name/Paketlage und Security-/Signaturkanäle müssen vor öffentlichem 0.1 bestätigt werden. |

Details, betroffene Pfade und Resolution stehen in `findings.yaml`.

## Empfohlene nächste Reihenfolge

1. `IAM-026`: atomare Patch-/Revoke-Idempotenz auf der nun vollständigen Create-Ablage ergänzen.
2. `IAM-025`, danach `IAM-013`: frameworkfreie CRUD-/Bearer-Use-Cases und anschließend die OpenAPI-konforme HTTP-Runtime umsetzen.
3. `MON-010`, `MON-011`, `DATA-010`, `API-010`, `WEB-010`: Monitor-CRUD als erster vollständiger öffentlicher Vertikalschnitt.
4. `CHECK-010` bis `CHECK-012`, `ALERT-010`, `DATA-011`, `WEB-011`: erster echter HTTP-Pfad einschließlich ehrlicher Fehlerklassifikation und atomarer Outbox; erst danach weitere 0.1-Features und Release-Hardening.

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

Der unabhängige Validator wiederholte die vollständigen aktuellen Repository-Gates für Commit `4ef411a4718d21fc4f364494dc3810f716215e98` aus einem sauberen Detached-HEAD-Checkout. Die Suite lief gegen das gepinnte Image `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` auf Loopback und bestand ohne Skip. Details, exakte Befehle und verbleibende Release-Lücken stehen in `docs/implementation-evidence/evid-001-independent-head-validation.md`; CI-basierte Freigabe ist weiterhin nicht behauptet.

## Abschluss von EVID-001

`EVID-001` ist gelöst. Commit `4ef411a4718d21fc4f364494dc3810f716215e98` bestand aus einem separaten sauberen Checkout alle aktuellen Contract-, Tracking-, Generated-, Secret-, Tool-, Rust-, PostgreSQL-/SQLite-, Supply-Chain-, Web-, Browser- und Release-Build-Gates. Der absichtlich rote 0.1-Release-Runner bestätigt weiterhin ehrlich, dass alle 15 v0.1-Produktszenarien nur geplant sind. Coverage blieb unverändert; ausschließlich die Verification der acht vom Finding betroffenen Requirements wurde auf `independent` angehoben. Der historische fehlgeschlagene Bootstrap-Verdict wurde nicht überschrieben. Details stehen in `docs/implementation-evidence/evid-001-independent-head-validation.md`.

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

`IAM-010` ist `implemented`, nicht `verified`. Die frameworkfreie Domain bildet die standardmäßigen 12 Stunden Inaktivität und sieben Tage absolute Laufzeit, konfigurierbare validierte Grenzen, Aktivitätsverlängerung ohne Überschreiten des absoluten Ablaufs, Rotation nach Login/Rechteänderung/sensibler Aktion, Revoke bei Logout/Recovery sowie sessiongebundene CSRF-Entscheidungen deterministisch ab. Der OpenAPI-Vertrag verwendet für Auth-Fehler jetzt stabile, generische Codes und unterscheidet Recovery-Tokenfehler nicht nach Existenz, Ablauf oder Einmalverwendung. Sämtliche lokalen Gates einschließlich echtem PostgreSQL 16.9, SQLite, Supply Chain, Frontend, Playwright und Release-Build sind grün. Recovery und Laufzeit folgen in `IAM-014`/`IAM-012`; unabhängige commit-gebundene Review-/CI-Evidence fehlt weiterhin. Details stehen in `docs/implementation-evidence/iam-010-auth-domain-contract.md`.

## Abschluss von IAM-011

`IAM-011` ist nach der Größentrennung von Session-Lifecycle (`IAM-015`) und Recovery (`IAM-014`) `implemented`, nicht `verified`. Migration `0002` speichert UUIDv7-Sessions mit UTC-Mikrosekunden und ausschließlich typisierten SHA-256-Digests. PostgreSQL 16.9 und SQLite bestehen denselben Create-/Lookup-Vertrag einschließlich DB-Constraints, Audit-Redaktion und atomarem Create-Rollback. Alle lokalen Gates sind grün; unabhängige commit-gebundene Evidence fehlt. Details stehen in `docs/implementation-evidence/iam-011-session-persistence.md`.

## Abschluss von IAM-015

`IAM-015` ist `implemented`, nicht `verified`. PostgreSQL 16.9 und SQLite bestehen denselben Session-Lifecycle-Vertrag: Refreshes sind aktiv, monoton, ablaufgeschützt und optimistisch versioniert; konkurrierende, veraltete, rückwärts datierte, abgelaufene oder widerrufene Schreibversuche bleiben ohne Wirkung. Revoke und kohärentes redigiertes Audit committen oder rollen gemeinsam zurück. Alle lokalen Gates sind grün; Details stehen in `docs/implementation-evidence/iam-015-session-lifecycle.md`.

## Abschluss von IAM-014

`IAM-014` ist `implemented`, nicht `verified`. PostgreSQL 16.9 und SQLite bestehen denselben Recovery-Vertrag mit Hashspeicherung, Ablauf, genau einmaligem Verbrauch und atomarer Passwort-Ersetzung, Session-Revoke- und Auditwirkung. Alle lokalen Gates sind grün; Details stehen in `docs/implementation-evidence/iam-014-recovery-persistence.md`.

## Abschluss von IAM-021

`IAM-021` ist nach der Größentrennung von Storage (`IAM-023`), Lifecycle (`IAM-022`) und HTTP-Runtime (`IAM-013`) `implemented`, nicht `verified`. Die frameworkfreie Domain-/Application-Grenze validiert API-Token-Arten, exakte Scopes ohne Rechteableitung, kanonische IPv4-/IPv6-CIDRs, Ablauf/Revoke und IP-Bindung. Tokenwerte behalten hinter dem separaten Lookup-Präfix 256 Bit geheime Entropie; Raw-Werte und Argon2id-Hashes sind in Debug-Ausgaben redigiert. Alle lokalen Gates sind grün; Persistenz, Auditwirkung, HTTP-CRUD und produktive Bearer-Authentifizierung folgen getrennt. Details stehen in `docs/implementation-evidence/iam-021-api-token-domain.md`.

## Abschluss von IAM-023

`IAM-023` ist nach der tatsächlichen Diffmessung separat vom Lifecycle-Paket `IAM-022` `implemented`, nicht `verified`. Migration `0004` speichert API-Token auf PostgreSQL und SQLite ausschließlich als sicheren Lookup-Präfix plus Argon2id-Hash; Create und redigiertes Audit committen oder rollen gemeinsam zurück. Beide Engines bestehen denselben Get-/Präfix-/Filter-/Cursor-Sortiervertrag, Schema-Wiederholung und Newer-Schema-Rejection. Patch, Revoke und Last-used folgen in `IAM-022`; HTTP bleibt in `IAM-013`. Alle lokalen Gates sind grün; Details stehen in `docs/implementation-evidence/iam-023-api-token-storage.md`.

## Abschluss von IAM-022

`IAM-022` ist `implemented`, nicht `verified`. Der gemeinsame PostgreSQL-16.9-/SQLite-Vertrag deckt optimistisch versioniertes Patch/Revoke, monotones Last-used, atomaren redigierten Audit-Rollback sowie Stale-, Replay-, Revoke-, Ablauf- und Rückwärtszeit-Negativfälle ab. Beim Fortsetzungsreview wurde ein Ablauf-Bypass gefunden und test-first geschlossen: Ein abgelaufenes Token kann weder gepatcht und durch Entfernen des Ablaufdatums reaktiviert noch nachträglich revokiert werden. Nach dem Rebuild ließ die Windows-Code-Integrity-Policy die neue Test-Binary zu; sämtliche lokalen Repository-Gates einschließlich der fünf echten CLI-Prozesstests sind grün. Unabhängige commit-gebundene Review-/CI-Evidence fehlt weiterhin. Details stehen in `docs/implementation-evidence/iam-022-api-token-lifecycle.md`.

## Abschluss von SPEC-020

`SPEC-020` ist `implemented`, nicht `verified`. Die Preflight-Nachprüfung ergab, dass das ursprünglich nächste, bereits am 800-Zeilen-Limit liegende Paket `IAM-013` wegen der zusätzlich erforderlichen, bislang ungeplanten Migration den zulässigen Umfang überschreiten würde. Es wurde deshalb vor der Umsetzung in `SPEC-020`, `IAM-024`, `IAM-025` und den verbleibenden HTTP-Slice `IAM-013` geteilt. Der Vertrag legt jetzt fest, dass ein identischer API-Token-Create-Replay 24 Stunden lang dieselbe `201`-Antwort einschließlich desselben Tokenwerts liefert, während derselbe Key mit abweichendem Request-Hash ohne Geschäfts- oder Auditwirkung als `409 idempotency_key_reused` scheitert. Die tokenhaltige Replay-Payload muss authentifiziert verschlüsselt, an Actor/Methode/Pfad/Request-Hash gebunden und von normalen Reads, Audit, Problems, Logs und Telemetrie ausgeschlossen sein. Alle lokalen Gates einschließlich echtem PostgreSQL 16.9, SQLite, Frontend, Playwright und Release-Build sind grün. Persistenz, Application/Bearer und HTTP-Runtime folgen in `IAM-024`, `IAM-025` und `IAM-013`; unabhängige commit-gebundene Review-/CI-Evidence fehlt. Details stehen in `docs/implementation-evidence/spec-020-api-token-idempotency-contract.md`.

## Abschluss von IAM-027

`IAM-027` ist `implemented`, nicht `verified`. Das ursprünglich ausgewählte `IAM-024` überschritt nach testweiser Vertikalimplementierung mit 888 handgeschriebenen Einfügungen das 800-Zeilen-Limit und wurde deshalb vor Abschluss in die AEAD-/Schema-Grundlage `IAM-027`, atomare Create-Persistenz `IAM-024` und Patch-/Revoke-Idempotenz `IAM-026` geteilt. `IAM-027` bindet Replay-Verschlüsselung typisiert an Key-Version, Actor, Methode, Pfad, Idempotency-Key und Request-Hash, verwendet zufällige Nonces und redigierte Debug-Grenzen und ergänzt engine-paritäre Migration `0005` mit exakt 24 Stunden Ablauf ohne Klartext-Payloadspalte. PostgreSQL 16.9, SQLite und alle lokalen Gates sind grün. Der während der Pflichtvalidierung gefundene High-Severity-`fast-uri`-Befund wurde test-first ohne Audit-Ausnahme behoben (`SEC-002`). Atomare Create-Operationen und Ablaufbereinigung sind nun in `IAM-024` implementiert; Patch/Revoke folgen in `IAM-026`. Details stehen in `docs/implementation-evidence/iam-027-api-token-idempotency-foundation.md`.

## Abschluss von IAM-024

`IAM-024` ist `implemented`, nicht `verified`. PostgreSQL 16.9 und SQLite reservieren API-Token-Create-Keys transaktional, committen Token, genau ein redigiertes Audit-Event und den verschlüsselten Replay-Datensatz gemeinsam und liefern bei identischem Hash denselben gespeicherten Ciphertext. Abweichende Hashes, fehlgeschlagene Writes und identische Replays haben keine Fach- oder Auditwirkung; abgelaufene Einträge werden nicht wiedergegeben und begrenzt bereinigt. Alle lokalen Gates sind grün; unabhängige commit-gebundene Review-/CI-Evidence fehlt. Details stehen in `docs/implementation-evidence/iam-024-api-token-idempotency-storage.md`.
