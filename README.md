# Takt

Takt ist eine self-hosted Monitoring- und Statusseiten-Plattform in aktiver
Entwicklung. Das Ziel ist ein schlankes Home-Lab-Erlebnis mit stabiler REST-API,
deklarativer Konfiguration, PostgreSQL-Unterstützung und verteilten Probes.

> [!IMPORTANT]
> Takt ist noch **vor Version 0.1** und derzeit kein einsatzbereites
> Monitoringprodukt. Der aktuelle Code liefert vor allem die abgesicherten
> Grundlagen für Identität, Sessions, API-Tokens, Persistenz und öffentliche
> Verträge. Den ehrlichen Implementierungsstand zeigt
> [`docs/implementation-tracking/current-status.md`](docs/implementation-tracking/current-status.md).

## Zielbild

Bis Version 0.3 soll Takt unter anderem Folgendes bieten:

- HTTP(S)-, TCP-, DNS-, ICMP-, TLS-, Push- und Browser-Checks
- Zustandsauswertung, Wartungsfenster, Uptime und Benachrichtigungen
- öffentliche Statusseiten
- Verwaltung über Web UI, REST API, `taktctl` und deklaratives `plan/apply`
- PostgreSQL für Produktion und SQLite für lokale Einzelinstanzen
- entfernte, per mTLS angebundene Probes
- ein einzelnes Server-Binary mit eingebetteter React-Oberfläche

Verbindlich ist dabei nicht diese Kurzfassung, sondern das
[`specs/`](specs/README.md)-Paket mit seinen versionierten Verträgen und
Abnahmeszenarien.

## Aktueller Stand

Bereits vorhanden:

- lokaler Serverstart mit automatisch migriertem SQLite
- Liveness- und Readiness-Endpunkte
- lokaler Administrator-Bootstrap
- Browser-Login, Session-Lesen und Logout mit sicheren Cookie-/CSRF-Grenzen
- API-Token-Domäne, Bearer-Authentifizierung sowie List/Get über die öffentliche API
- optimistisch versionierte und atomar auditierte API-Token-Create/Patch/Revoke-Persistenz
- identisches Domain- und Repository-Verhalten für PostgreSQL und SQLite
- validierte OpenAPI-, JSON-Schema-, Proto- und Gherkin-Verträge
- reproduzierbare Codegenerierung, Supply-Chain-Prüfungen und CI-Gates

Noch nicht als Produktpfad vorhanden:

- Monitorverwaltung und Check-Ausführung
- Scheduler, Evaluator, Uptime und Outbox
- Benachrichtigungen und Statusseiten
- vollständige Produktoberfläche
- Probe- und Browser-Worker
- CLI und deklaratives Apply
- API-Token-Schreiboperationen über die öffentliche HTTP-Runtime

## Schnellstart

### Voraussetzungen

- [Rustup](https://rustup.rs/) – das Repository wählt Rust `1.95.0`
  automatisch über `rust-toolchain.toml`
- Node.js `24.15.0`
- Corepack und pnpm `11.7.0`

```bash
corepack enable
corepack prepare pnpm@11.7.0 --activate
pnpm install --frozen-lockfile
pnpm build
cargo run --locked -p takt-server
```

Der lokale Server bindet ausschließlich an
[`http://127.0.0.1:8080`](http://127.0.0.1:8080). Im lokalen Profil liegt die
SQLite-Datenbank im plattformspezifischen Anwendungsdatenverzeichnis, nicht im
Repository.

```bash
curl http://127.0.0.1:8080/health/live
curl http://127.0.0.1:8080/health/ready
```

Wie der erste lokale Owner sicher über `--password-stdin` angelegt wird,
beschreibt [`docs/persistence.md`](docs/persistence.md#local-administrator-command).
Weitere Entwicklungs- und PostgreSQL-Hinweise stehen in
[`docs/development.md`](docs/development.md).

## Architektur

Takt bleibt bis einschließlich Version 0.3 ein modularer Monolith. Domänenlogik
kennt weder HTTP noch SQL oder Runtime-Frameworks; die öffentliche Web UI
kommuniziert ausschließlich über die API.

| Bereich | Verantwortung |
|---|---|
| `crates/domain` | Frameworkfreie Domänentypen, Invarianten und Fehler |
| `crates/application` | Use Cases, Ports, AuthZ-, Audit- und Idempotenzgrenzen |
| `crates/persistence` | SQLx-Repositories und gemeinsame PostgreSQL-/SQLite-Semantik |
| `crates/api` | Axum-Routen, HTTP-Validierung und Problem Details |
| `crates/server` | Produktionskomposition, Konfiguration und CLI |
| `crates/probe-protocol` | Generierte Typen des Probe-Protokolls |
| `web` | Strikte React-/TypeScript-Oberfläche und eingebetteter Build |

Die detaillierten Invarianten stehen in
[`specs/01-architecture.md`](specs/01-architecture.md).

## Repository-Struktur

```text
crates/       Rust-Domäne, Application, API, Persistenz und Server
web/          React-/TypeScript-Frontend
migrations/   Forward-only Migrationen für PostgreSQL und SQLite
specs/        Normative Spezifikation, Verträge und Acceptance-Szenarien
tools/        Generatoren sowie Architektur-, Contract- und Drift-Prüfungen
tests/e2e/    Browserbasierte End-to-End-Tests
docs/         Entwicklung, Betrieb, Tracking und Implementation Evidence
```

## Entwicklung und Tests

Schnelle, fokussierte Prüfungen:

```bash
cargo test -p takt-domain -p takt-application
pnpm test --run
pnpm contracts:validate
pnpm check:architecture
pnpm check:tracking
```

Vor einer Übergabe müssen zusätzlich die vollständigen Repository-Gates laufen.
Der Workspace-Test benötigt eine echte PostgreSQL-16.9-Instanz und
`TAKT_TEST_POSTGRES_URL`; die reproduzierbare Docker-Konfiguration und alle
Befehle stehen unter
[`docs/development.md#complete-local-gates`](docs/development.md#complete-local-gates).

Die 37 Acceptance-Szenarien besitzen bereits ein geprüftes Binding-Inventar,
sind aber noch nicht runnable. `pnpm acceptance:run -- --release v0.1` schlägt
deshalb derzeit bewusst fehl und darf nicht als bestandene Produktabnahme
interpretiert werden.

## Verträge und Arbeitsweise

Die Rangfolge bei widersprüchlichen Quellen ist:

1. maschinenlesbare Verträge in `specs/contracts/`
2. Acceptance-Szenarien in `specs/acceptance/`
3. Release-Exit-Kriterien
4. nummerierte Produkt- und Architekturspezifikationen
5. Beispiele

Änderungen werden als kleine, test-first umgesetzte Arbeitspakete geführt.
Vor Beiträgen bitte zuerst
[`AGENTS.md`](AGENTS.md),
[`specs/README.md`](specs/README.md) und
[`docs/implementation-tracking/README.md`](docs/implementation-tracking/README.md)
lesen. Bekannte Lücken werden in
[`docs/implementation-tracking/findings.yaml`](docs/implementation-tracking/findings.yaml)
explizit nachverfolgt.

## Sicherheit und Daten

- Secrets gehören weder in API-Antworten noch in Logs, Audit, Exporte,
  Metriken, Traces oder Test-Fixtures.
- SQL verwendet gebundene Parameter; veröffentlichte Migrationen sind
  unveränderlich und werden nur vorwärts ergänzt.
- Fachschreibvorgang, Zustandsübergang, Audit und Outbox müssen dort, wo sie
  gemeinsam auftreten, atomar gespeichert werden.
- Ein Infrastruktur- oder Probe-Fehler darf niemals als Fehler des überwachten
  Ziels klassifiziert werden.
- PostgreSQL ist für Produktion vorgesehen; SQLite dient lokalen
  Einzelinstanzen und Evaluation.

Das vollständige Sicherheits- und Betriebsmodell steht in
[`specs/06-security-operations.md`](specs/06-security-operations.md).

## Roadmap

- **0.1:** zentraler Monitoring-Server, Kernchecks, API/CLI, UI,
  Benachrichtigungen und öffentliche Statusseite
- **0.2:** Migration, Backups/Restore, Datenexport und Betriebsreife
- **0.3:** verteilte Probes, Browser-Checks, OIDC und erweiterte Auswertung

Details und Exit-Kriterien:
[`specs/08-roadmap.md`](specs/08-roadmap.md).

## Lizenzstatus

Die Cargo-Metadaten verwenden derzeit `Apache-2.0` als Spezifikationsannahme.
Vor einer öffentlichen Veröffentlichung müssen Lizenzmodell, Name,
Paketregistrierung und offizielle Security-/Signaturkanäle noch durch den
Projekteigentümer bestätigt werden. Das offene Owner-Thema ist als `DEC-001`
in [`findings.yaml`](docs/implementation-tracking/findings.yaml) dokumentiert.
