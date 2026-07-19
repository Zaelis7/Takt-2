# 07 – Qualität, Tests und Abnahme

## 1. Qualitätsmodell

Takt gilt nicht als fertig, weil es kompiliert oder eine Demo funktioniert. Freigabe erfordert Belege für:

1. fachliche Korrektheit
2. Vertragstreue
3. Sicherheit und Mandantentrennung
4. Datenintegrität und Upgradefähigkeit
5. Zuverlässigkeit unter Fehlern
6. Bedienbarkeit und Barrierefreiheit
7. messbare Kapazität

## 2. Testebenen

### Unit- und Property-Tests

Pflicht für:

- Alert-Zustandsautomat und Schwellen
- Quorum-Auswertung mit fehlenden/verspäteten Probes
- Uptime-Zeitgewichtung, Wartung und `UNKNOWN`
- Scheduler-Zeitberechnung, Jitter, Lease und Zeitumstellung
- Secret-Redaction
- Rollen-/Permission-Matrix
- Config-Normalisierung und Diff
- Retry-/Backoff-Berechnung

Property-Tests erzeugen Reihenfolgen von Observationen, Zeitintervallen und Probe-Zuständen. Invarianten:

- interne Fehler allein erzeugen nie `DOWN`
- Uptime-Anteile liegen zwischen 0 und 1
- eingeschlossene plus ausgeschlossene Dauer überschreitet Gesamtdauer nicht
- dieselbe Observation ändert Zustand höchstens einmal
- idempotentes Apply erzeugt nach dem ersten Lauf keine Änderung

### Contract-Tests

- OpenAPI ist syntaktisch gültig und nicht breaking gegenüber dem letzten Release.
- Jede Operation hat Auth-, Permission-, Erfolgs- und Problem-Response-Tests.
- Laufzeitantworten werden gegen OpenAPI validiert.
- JSON-Schema akzeptiert alle Beispiele und verwirft gezielte ungültige Fixtures.
- Proto kompiliert; aktuelle Serverversion kommuniziert mit aktueller und vorheriger Minor-Probe.
- CloudEvents und Webhook-Signaturen besitzen Golden Tests.

### Datenbanktests

Jeder Repository-Vertrag läuft unverändert gegen PostgreSQL und SQLite. PostgreSQL ist in CI ein echter Dienst, kein Mock.

Pflichtfälle:

- konkurrierende Leases
- ETag-/Versionskonflikte
- Transaktion aus Evaluation, State Transition und Outbox
- Retention während Schreiblast
- Cursor-Pagination bei gleichen Zeitstempeln
- Mandantenfilter und negative Cross-Tenant-Zugriffe
- Migration von jeder unterstützten Release-Fixture
- Backup/Restore mit Secrets und Rollups

### Integrationstests

Kontrollierte Zielserver simulieren DNS, TCP, TLS, Redirects, langsame Antworten, große Bodies, Verbindungsabbrüche und fehlerhafte Protokolle. Tests dürfen nicht vom öffentlichen Internet abhängen.

### Ende-zu-Ende

Playwright führt die wichtigsten Nutzerpfade gegen einen echten Server und echte Datenbank aus:

- Erstsetup und erster Monitor
- Monitor testen, speichern, auslösen und Zustand sehen
- Notification Channel testen
- Statusseite veröffentlichen und öffentlich lesen
- deklarativen Plan und Apply prüfen
- Probe enrollen und Multi-Standort-Ergebnisse sehen
- Incident und Maintenance
- OIDC/RBAC mit Test-Identity-Provider
- Upgrade einer persistierten Vorversion

### Chaos- und Resilienztests

- Datenbank während Check-Auswertung nicht erreichbar
- Serverneustart zwischen Observation und Notification
- Probe 15 Minuten offline mit gefüllter Queue
- doppelte, verspätete und ungeordnete Observationen
- Notification Endpoint liefert 429, 500 und Timeout
- Systemuhr springt im Test kontrolliert vor/zurück
- Datenträger voll für Probe-Queue und SQLite

Erwartung ist Datenintegrität und ehrlicher Systemzustand, nicht zwingend lückenloser Dienst unter jedem Fehler.

### Sicherheitsprüfungen

- SAST, Dependency-/Container-Scan und Secret-Scan
- DAST gegen API und öffentliche Seite
- Autorisierungsmatrix mit negativen Tests
- SSRF-Testkorpus inklusive Redirect, DNS Rebinding, IPv6 und Metadata-Adressen
- XSS-Testkorpus in Namen, Incident-Text und Zielantworten
- CSRF, Session-Fixation, Rate Limits und Tokenrotation
- Browserworker-Isolationsprüfung
- Fuzzing für öffentliche Parser, Proto-Ingest und URL-Normalisierung

## 3. Coverage und Mutation

- `domain` und `application`: mindestens 85 % Line Coverage und 80 % Branch Coverage.
- Gesamt-Rust ohne generierten Code: mindestens 75 % Line Coverage.
- Frontend-Fachlogik: mindestens 80 % Line Coverage.
- Kritische Zustandsautomaten, Permission-Checks und Config-Diff erhalten Mutationstests mit mindestens 80 % Mutation Score.

Coverage darf niemals durch Ausschluss produktiver Dateien künstlich erhöht werden. Prozentwerte ergänzen fachliche Fälle und ersetzen sie nicht.

## 4. Statische Qualitätsgates

Rust:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
cargo audit
```

Frontend:

```text
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test --run
pnpm build
pnpm playwright test
```

Zusätzlich:

- keine `TODO`, `FIXME`, `todo!()`, `unimplemented!()` oder `panic!()` auf produktiven Releasepfaden
- keine neuen Warnungen
- keine unformatierte Migration oder Vertragsdatei
- keine Snapshot-Aktualisierung ohne lesbaren Review-Diff
- generierter Code wird reproduzierbar erzeugt und auf Drift geprüft

## 5. Performance- und Zuverlässigkeitstests

Die Ziele aus `04-probes-and-checks.md` werden pro Release Candidate mindestens 30 Minuten unter konstanter Last und 10 Minuten Burst getestet.

Zusätzliche SLOs im Referenztest:

- Verwaltungs-API p95 unter 300 ms und p99 unter 1 s bei 50 parallelen Clients, ausgenommen Exporte/Operations
- öffentliche Statusprojektion p95 unter 200 ms aus warmem Cache
- angenommene Observationen: null Verlust bei normalem Shutdown und Serverneustart
- Outbox: 99 % erfolgreicher lokaler Webhook-Zustellungen innerhalb 10 s ohne Backlog
- Speichernutzung steigt im vierstündigen Soak-Test nicht dauerhaft um mehr als 10 % nach Warm-up

Messung muss Setup, Commit, Hardware, Datensatz und Rohresultate dokumentieren.

## 6. Kompatibilitätsmatrix

| Kombination | Pflicht |
|---|---|
| PostgreSQL 16 und aktuelle 17/18-Version | ja, soweit zum Release verfügbar |
| SQLite gebündelte Version | ja |
| Linux amd64/arm64 Binary | ja |
| OCI Container amd64/arm64 | ja |
| Chromium, Firefox, WebKit für Admin-UI | letzte zwei stabile Major-Versionen |
| Probe N mit Server N | ja |
| Probe N-1 mit Server N | ja |
| Upgrade 0.1 → 0.2 → 0.3 | ja |

## 7. Definition of Done für ein Arbeitspaket

Ein Paket ist nur fertig, wenn:

- zugehörige Requirement-IDs genannt und erfüllt sind
- Verträge vor oder zusammen mit Code aktualisiert sind
- positive, negative und Berechtigungstests existieren
- beide Datenbanken getestet sind, falls Persistenz betroffen ist
- UI Lade-, Leer-, Fehler- und Erfolgzustand enthält, falls UI betroffen ist
- Logs, Metriken und Auditwirkung bewertet wurden
- keine Secrets in Fixtures oder Ausgaben stehen
- Dokumentation und Upgradehinweise aktuell sind
- ein unabhängiger Review-Agent den Change gegen die Spec geprüft hat
- CI vollständig grün und der Ergebnisnachweis gespeichert ist

## 8. Release-Abnahme

Ein Release Candidate benötigt ein unveränderliches Evidence Bundle:

- Commit SHA und Toolchain-Versionen
- CI-Übersicht
- Unit/Integration/E2E/Contract-Ergebnisse
- Coverage- und Mutation-Bericht
- Security-/Dependency-Scan und Ausnahmen
- Migration-, Backup- und Restore-Bericht
- Performance- und Soak-Bericht
- SBOM, Checksummen und Signaturbeleg
- Liste bekannter Einschränkungen
- manuelle Smoke-Checkliste für den Eigentümer

## 9. Manuelle Eigentümertests

Da der Projekteigentümer nur das Output testen möchte, wird dessen Testumfang bewusst kurz gehalten:

1. Installation mit dokumentiertem Einzeiler
2. ersten Monitor über UI erstellen
3. denselben Monitor über CLI/API ändern
4. echten Ausfall und Recovery beobachten
5. Statusseite auf Desktop und Mobilgerät prüfen
6. Backup erzeugen, neue Instanz wiederherstellen
7. Upgrade vom vorherigen Release durchführen

Alles Weitere muss automatisiert belegt sein. Ein manueller Eigentümertest darf kein Ersatz für fehlende Regressionstests werden.

## 10. Release-Blocker

- reproduzierbarer Datenverlust oder falscher Cross-Tenant-Zugriff
- interner Fehler erzeugt falschen Zielausfall
- fehlgeschlagenes unterstütztes Upgrade/Restore
- kritischer oder hoher unakzeptierter Sicherheitsbefund
- OpenAPI/Schema/Proto und Laufzeit driften auseinander
- instabiler Test wird deaktiviert statt ursächlich behoben
- nicht redigiertes Secret in einer Oberfläche oder Diagnoseausgabe
- Muss-Abnahmeszenario ist rot oder übersprungen
