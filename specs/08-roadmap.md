# 08 – Roadmap bis Takt 0.3

## 1. Lieferstrategie

Jede Version wird in vertikalen, nutzbaren Schnitten gebaut. Eine Version ist erst abgeschlossen, wenn ihre Exit-Kriterien erfüllt sind; eine bloße Ansammlung implementierter Features reicht nicht.

## 2. Takt 0.1.0 – Solider API-first Kern

### Produktumfang

- Einzelnutzer-/lokales Admin-Setup mit sicherer Session
- interne Default-Organisation und Default-Projekt
- API-Tokens und Scopes
- HTTP(S), TCP, DNS, ICMP, TLS und Push-Monitore
- Scheduler, lokaler Executor, Evaluator, Zustandsautomat
- PostgreSQL und SQLite
- Monitor-UI, Verlauf, zeitgewichtete Uptime
- REST API `/api/v1` und OpenAPI
- `taktctl validate`, `plan`, `apply`, `export`, `monitor run`
- Benachrichtigungen: Webhook, SMTP, Slack, Discord, Telegram
- transaktionale Outbox und Zustellversuche
- einfache öffentliche/token-geschützte Statusseiten
- Deutsch, Englisch, Light/Dark Mode und grundlegende Barrierefreiheit
- Container und Linux-Binaries für amd64/arm64
- strukturierte Logs, Metrics, Health/Readiness
- Backup-/Restore-Befehle für SQLite und Dokumentation für PostgreSQL

### Empfohlene Implementierungsreihenfolge

1. Repository, CI, Toolchains, Domain IDs und Fehlervertrag
2. Datenbankabstraktion, erste Migration, lokale Admin-Identität
3. Monitor CRUD über API und minimale UI
4. HTTP-Check als erster vollständiger Pfad: schedule → execute → evaluate → store → live UI
5. Zustandsautomat, Uptime und Outbox
6. Webhook als erster Notification Adapter
7. übrige 0.1-Checks und Kanäle über vorhandene Verträge
8. deklarative Config und CLI
9. Statusseite
10. Hardening, Migration-/Restore-, E2E-, Last- und Security-Abnahme

### Exit-Kriterien 0.1

- Alle `@v0.1`-Szenarien in `acceptance/` grün.
- 1.000 60-Sekunden-Monitore erfüllen die Kapazitätsziele.
- Dieselben Repository- und API-Suites laufen auf PostgreSQL und SQLite.
- Interner DB-Ausfall wird sichtbar, erzeugt aber keinen falschen `DOWN`.
- Ein deklaratives Dokument mit 500 Monitoren ist beim zweiten Apply ohne Diff.
- Frische Installation, Backup, Restore und erneuter Start sind dokumentiert und automatisiert getestet.
- Kein kritischer/hoher Sicherheitsbefund.

## 3. Takt 0.2.0 – Verteilter und operativer Betrieb

### Produktumfang

- `takt-probe` mit Enrollment, mTLS, Rotation, Heartbeat und Offline-Queue
- Labelauswahl und Policies `any`, `all`, `quorum`
- Probe-Matrix und Standortzustände in UI/API
- Acknowledgements, Silences und Flapping-Drosselung
- einmalige und wiederkehrende Wartungsfenster
- manuelle Incidents mit unveränderlicher Timeline
- erweiterte Statusseiten und frei wählbare Uptime-Zeiträume
- Uptime-Kuma-Importer mit `analyze`, `plan`, `apply`
- belastbare Retention/Rollups und PostgreSQL-Partitionen
- Event-Webhooks im CloudEvents-Format

### Reihenfolge

1. Proto und Probe-Simulator
2. Enrollment und Verbindungsstatus
3. Job-Leases/Dispatch und idempotenter Observation-Ingest
4. Offline-Puffer und Zertifikatsrotation
5. Multi-Probe-Auswertung und UI
6. Acknowledge/Silence/Maintenance/Flapping
7. Incidents und Statusseiten
8. Importer
9. Last-, Disconnect-, Upgrade- und Security-Abnahme

### Exit-Kriterien 0.2

- Alle 0.1- und `@v0.2`-Szenarien grün.
- Fünf Probes und 5.000 60-Sekunden-Monitore erfüllen die Kapazitätsziele.
- 15 Minuten Verbindungsabbruch verlieren keine angenommene Observation.
- Doppelte und ungeordnete Observationen erzeugen höchstens eine Zustandswirkung.
- Probe-Ausfall führt korrekt zu `UNKNOWN` und nicht automatisch `DOWN`.
- Importer analysiert unterstützte Uptime-Kuma-Fixture, meldet Verluste und ist beim erneuten Apply idempotent.
- Upgrade 0.1 → 0.2 einschließlich Restore-Fallback ist erfolgreich.

## 4. Takt 0.3.0 – Team-, Plattform- und Erweiterungsreife

### Produktumfang

- sichtbare Organisationen und Projekte
- Rollen `owner`, `admin`, `editor`, `operator`, `viewer`
- Einladungen/Mitgliedschaften und serverseitige Mandantentrennung
- OIDC Authorization Code mit PKCE und Gruppenmapping
- vollständiges Audit-Log und Export
- E-Mail-Abonnenten für Statusseiten mit Double-Opt-in/Abmeldung
- Terraform Provider für Organisation, Projekt, Monitor, Statusseite, Maintenance und Kanäle ohne Secret-Readback
- Kubernetes Discovery für opt-in Services und Ingresses
- Monitorabhängigkeiten zur Unterdrückung von Folgealarmen, ohne Zustände zu verfälschen
- isolierte deklarative Browserchecks
- stabile deutsch-/englischsprachige Admin-UI für den vollständigen Umfang
- Release-Signaturen, SBOM und gehärtete Produktionsprofile

### Kubernetes Discovery

- liest nur Namespaces mit expliziter Konfiguration
- Ressourcen benötigen Opt-in Label/Annotation
- erzeugt declarative/discovery-managed Monitore mit stabiler Herkunft
- zeigt Plan vor Löschung; Grace Period mindestens zwei Discovery-Zyklen
- benötigt nur lesende Kubernetes-Rechte
- kein Kubernetes Operator in 0.3

### Monitorabhängigkeiten

- gerichteter azyklischer Graph; Zyklen werden beim Schreiben abgelehnt
- Abhängigkeit unterdrückt Benachrichtigungen eines nachgelagerten Monitors, wenn dessen Ausfall plausibel Folge ist
- echter Zustand und Observation bleiben sichtbar
- UI erklärt die unterdrückende Abhängigkeit
- maximal 100 Kanten pro Monitor und 20 Ebenen

### Exit-Kriterien 0.3

- Alle Szenarien 0.1 bis 0.3 grün.
- Negative Cross-Tenant-Matrix deckt jede Ressourcenart und Rolle ab.
- OIDC Login, Logout, abgelaufene Tokens und geänderte Gruppen sind E2E getestet.
- Terraform `plan/apply/plan` endet ohne Drift; Import vorhandener Ressourcen ist dokumentiert.
- Discovery erstellt, aktualisiert und entfernt Testressourcen erst nach Grace Period.
- Browserworker besteht Isolationstest und kann weder Hostdatei noch Metadata-Endpunkt erreichen.
- E-Mail-Abonnement speichert erst nach Double-Opt-in aktiv und löscht/entkoppelt bei Abmeldung.
- Upgrade 0.1 → 0.2 → 0.3 sowie Backup/Restore unter 0.3 ist erfolgreich.
- Release Evidence Bundle aus `07-quality-acceptance.md` ist vollständig.

## 5. Nicht in 0.3 hineinziehen

- frei programmierbare Plugins oder beliebiger JavaScript-/Shell-Code
- native Apps
- eigener Pager-/On-call-Dienst
- Prometheus-/OpenTelemetry-Metrik-Ingestion als Monitoringplattform
- Control-Plane-HA oder Sharding
- kommerzielle Billing-Logik
- 90+ Notificationskanäle
- MySQL/MariaDB-Unterstützung

Diese Punkte werden nach 0.3 anhand realer Nutzung priorisiert, nicht vorweg implementiert.

## 6. Versionierung und Support

- `0.x` darf intern noch zügig entwickelt werden, aber `/api/v1` bleibt ab öffentlichem 0.1 nach den Kompatibilitätsregeln stabil.
- Jede Minor-Version erhält Security Fixes bis drei Monate nach Erscheinen der Nachfolgeversion.
- Datenbankformat ist nie rückwärtskompatibel zu älteren Servern versprochen; Rollback erfolgt per Backup.
- Probe N-1 bleibt mit Server N kompatibel.
- Release Candidates heißen `0.x.0-rc.N` und durchlaufen dieselbe Evidence Pipeline wie final.

## 7. Issue-Struktur

Epics folgen diesen Präfixen:

- `CORE`: Domain, Zeit, IDs, Fehler
- `DATA`: Persistenz, Migration, Retention
- `API`: REST, AuthN/AuthZ, Verträge
- `CHECK`: Scheduler und Checktypen
- `PROBE`: Remote-Ausführung
- `ALERT`: Zustand, Outbox, Notifications
- `STATUS`: Statusseiten und Incidents
- `AUTO`: CLI, Config, Terraform, Discovery
- `WEB`: Admin-UI und Accessibility
- `OPS`: Packaging, Security, Observability, Backup

Jedes Issue gehört genau zu einem Release und enthält Requirement-IDs sowie messbare Abnahme. Horizontale „Backend fertig“-Epics ohne nutzbaren vertikalen Pfad sind zu vermeiden.
