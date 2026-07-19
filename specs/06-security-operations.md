# 06 – Sicherheit und Betrieb

## 1. Bedrohungsmodell

Takt verarbeitet hochsensible Informationen: interne Hostnamen, Netzwerkpfade, Tokens, Benachrichtigungsziele, Nutzeridentitäten und mögliche Response-Inhalte. Besonders relevant sind:

- unberechtigter Zugriff zwischen Organisationen/Projekten
- SSRF und Zugriff auf Cloud-Metadata oder Control-Plane
- Secret-Leaks in Logs, API, Audit, Exports oder Screenshots
- Kontoübernahme und Token-Diebstahl
- manipulierte oder kompromittierte Remote-Probes
- gespeicherte und reflektierte XSS über Namen, Incidents oder Zielantworten
- SQL Injection und unsichere dynamische Filter
- Ressourcenerschöpfung durch Checks, Push oder öffentliche Seiten
- Supply-Chain-Angriffe auf Rust-, npm-, Container- und Release-Artefakte
- Missbrauch eines Browserchecks als allgemeine Ausführungsumgebung

Vor jedem Minor-Release MUSS das Threat Model gegen neue Datenflüsse aktualisiert werden.

## 2. Sicherheitsvorgaben

### Eingaben und Ausgabe

- Alle externen Eingaben werden an der Systemgrenze validiert und größenbegrenzt.
- SQL verwendet gebundene Parameter; dynamische Spalten/Sortierung nur aus Allow-Lists.
- Nutzertexte werden als Text gerendert. Markdown, falls später unterstützt, wird serverseitig sanitisiert und erlaubt kein Roh-HTML.
- Content Security Policy ohne `unsafe-eval`; Nonces statt globalem `unsafe-inline`.
- Öffentliche und private Antworten setzen passende MIME-, Frame-, Referrer- und Sniffing-Schutzheader.

### Passwörter und Sitzungen

- Passwörter: Argon2id mit zur Release-Hardware kalibrierten Parametern, Passwortlänge mindestens 12, maximal 1024 Bytes.
- Keine erzwungenen periodischen Passwortwechsel.
- Login-Antwort verrät nicht, ob ein Konto existiert.
- Recovery ab 0.1 erfolgt über einmalige, kurzlebige Tokens; ist SMTP nicht konfiguriert, über expliziten lokalen Admin-CLI-Flow mit Audit.
- Session-Inaktivität standardmäßig 12 Stunden, absolute Laufzeit 7 Tage; anpassbar.

### OIDC in 0.3

- Authorization Code mit PKCE, `state` und `nonce`
- strikte Issuer-, Audience-, Signatur- und Zeitprüfung
- Gruppenmapping ist explizit; unbekannte Gruppen erhalten keine Rechte
- lokaler Break-glass-Administrator kann deaktiviert werden, aber die Konsequenz muss bestätigt sein

### Secrets

- Umsetzung gemäß `02-domain-data.md`
- Zentraler Redaction Layer für Logs, Problems, Audit und Telemetrie
- Debug-Logging darf Header/Body nicht pauschal ausgeben
- Konfigurations-Export referenziert Secret-Namen, nie Werte
- Support-Bundle ist vor Erzeugung und nach Erzeugung automatisiert auf bekannte Secret-Muster zu prüfen

## 3. Netzwerk- und Probe-Sicherheit

- Server lauscht standardmäßig auf Loopback, sofern kein Containerprofil aktiv ist.
- TLS-Terminierung darf extern erfolgen; direkte TLS-Konfiguration wird unterstützt.
- Probe-Verkehr ist mTLS-gesichert; Enrollment-Codes sind einmalig, kurzlebig und rate-limited.
- Probe-Identität ist Organisation und Zertifikat zugeordnet. Labels allein autorisieren keinen Zugriff.
- Server sendet nur Jobs, deren Projekt/Probe-Zuordnung autorisiert ist.
- Probe aktualisiert sich nicht ungefragt selbst. Binärupdates sind signiert und ein expliziter Betriebsprozess.
- Egress-Regeln aus `04-probes-and-checks.md` sind Pflicht.

## 4. Browsercheck-Isolation

- eigener Prozess oder Container, niemals Chromium im Serverprozess
- non-root User, seccomp/AppArmor sofern Plattform unterstützt, read-only Root-FS, begrenztes tmpfs
- CPU-, Speicher-, Prozess- und Zeitlimits
- kein Docker Socket, keine Host-Netzwerkfreigabe, keine sensitiven Mounts
- Downloads, Extensions, WebRTC, Geolocation und Clipboard standardmäßig deaktiviert
- Artefakte verschlüsselt, berechtigungsgeprüft und kurz aufbewahrt

## 5. Audit

Auditpflichtig sind mindestens:

- Login-Erfolg/-Fehler und Logout
- Token, Nutzer, Mitgliedschaft, Rolle und OIDC-Konfiguration
- Create/Update/Delete/Pause von Ressourcen
- Secret-Erzeugung/-Rotation ohne Wert
- Enrollment, Sperrung und Zertifikatsrotation von Probes
- Config plan/apply/prune und Import
- Backup, Restore, Migration und Retention-Konfigurationsänderung
- Incident-Veröffentlichung und Abonnentenexport

Audit-Datensätze sind append-only auf Anwendungsebene. Hash-Verkettung pro Organisation SOLL Manipulationen erkennbar machen. Eigentümer können Audit exportieren; Löschung folgt einer expliziten Retention Policy.

## 6. Datenschutz

- Datensparsamkeit: nur notwendige Ziel- und Kontaktdaten.
- Status-Abonnenten verwenden Double-Opt-in und nachweisbare Einwilligungszeitpunkte.
- Abmeldelinks sind signierte Einmal-/Scoped-Tokens und funktionieren ohne Login.
- E-Mail-Adressen werden in UI und Logs maskiert, außer ein berechtigter Admin öffnet die Detailansicht.
- Projekt-/Organisationslöschung bietet Export, Bestätigung und dokumentierte Löschfrist.
- Externe Telemetrie ist standardmäßig aus.

## 7. Supply Chain und Releases

- `Cargo.lock` und Frontend-Lockfile werden committed; keine lockeren Wildcard-Versionen.
- CI führt License-, Vulnerability- und Herkunftsprüfung aus.
- `unsafe` ist in eigenen Crates durch `#![forbid(unsafe_code)]` verboten, sofern kein genehmigter ADR mit Isolation und Tests existiert.
- Release-Builds sind reproduzierbar soweit Toolchains dies erlauben.
- Jedes Release enthält SHA-256-Prüfsummen, SBOM im CycloneDX- oder SPDX-Format und Sigstore/cosign-Signatur.
- Container läuft non-root, besitzt read-only-fähiges Root-FS und deklarierten Healthcheck.
- Kritische Abhängigkeitsschwachstellen blockieren Releases; Ausnahmen brauchen Ablaufdatum, Begründung und kompensierende Maßnahme.

## 8. Konfiguration und Start

Konfigurationsreihenfolge von niedrig zu hoch:

1. eingebaute Defaults
2. Konfigurationsdatei
3. Umgebungsvariablen `TAKT_*`
4. explizite CLI-Flags

Secrets SOLLEN über `*_FILE` oder Secret-Provider bezogen werden. Startlogs zeigen effektive nicht sensible Einstellungen und Quellen, nie Secret-Werte.

Pflichtkonfiguration Produktion:

- öffentliche Basis-URL
- Datenbank-URL/Secret
- Master-Key-Quelle
- Session-/Token-Key-Quelle
- Trusted-Proxy-Netze, falls Reverse Proxy
- Datenaufbewahrung und Backup-Hinweis

Unsichere Kombinationen führen in Produktion zum Startabbruch, nicht nur zu einer Warnung.

## 9. Health und Observability

- `/health/live`: Prozess kann Anfragen bearbeiten; keine Abhängigkeiten.
- `/health/ready`: Datenbank erreichbar, Migration passend, Schlüssel geladen, Kernworker gestartet.
- Öffentliche Health-Endpunkte zeigen keine Version oder internen Fehlerdetails.
- `/metrics` ist standardmäßig nur intern/geschützt erreichbar.
- Strukturierte Logs enthalten Zeit, Level, Service, Request-/Trace-ID und stabile Eventcodes.
- Metriken vermeiden hohe Kardinalität: Monitor-ID erscheint nicht standardmäßig als Label.

Pflichtmetriken:

- Scheduler-Lag und fällige Jobs
- Checkdauer/-ausgang aggregiert nach Typ und Probe
- Ingest- und Evaluierungsfehler
- Probe-Verbindungen und Offline-Queue
- Outbox-Backlog, Zustelllatenz und Fehler
- DB-Pool, Query-Latenz und Retention
- HTTP-Rate, Latenz und Statusklassen

## 10. Backup und Restore

### PostgreSQL

- Takt liefert dokumentierte Hooks/Kommandos für konsistente `pg_dump`- oder physische Backups, implementiert aber keine eigene Datenbank-Backup-Engine.
- Verschlüsselungs-Master-Key und Konfiguration müssen separat gesichert werden.
- `taktctl admin backup verify` prüft Metadaten, Schema-Kompatibilität und Vorhandensein der benötigten Schlüssel, ohne Restore zu ersetzen.

### SQLite

- Online Backup API oder kontrollierter Checkpoint; einfaches Kopieren einer aktiven WAL-Datenbank ist unzulässig.
- `taktctl admin backup create --output ...` erzeugt ein konsistentes Paket mit Manifest und Prüfsumme.

### Restore

- Restore erfolgt in eine leere Instanz oder nach expliziter Bestätigung einer stillgelegten Instanz.
- Restore-Test ist Bestandteil jedes Releasekandidaten.
- RPO/RTO sind Betreiberentscheidungen; Beispielprofile für Home-Lab und Produktion werden dokumentiert.

## 11. Upgrades

- Unterstützt werden direkte Upgrades von der vorherigen Minor-Version; 0.1 → 0.3 kann über 0.2 erfolgen oder wird explizit getestet.
- Server prüft vor Migration freien Speicher, Schema, Datenbankversion und Backup-Bestätigung.
- Während inkompatibler Migration bleibt Readiness rot.
- Gemischte Serverversionen sind bis 0.3 nicht unterstützt; paralleler HA-Betrieb ist kein Ziel.
- Probe kann eine Minor-Version hinter dem Server liegen.
- Release Notes listen Schemaänderungen, Konfigurationsänderungen, bekannte Risiken und Rollbackweg.

## 12. Betriebsprofile

### Home

- einzelner Server, eingebettete UI, SQLite, lokale Checks
- ein Volume für Daten und Schlüssel
- automatische Migration und tägliches SQLite-Backup

### Production

- Server mit PostgreSQL, externe TLS-Terminierung oder direktes TLS
- mindestens eine separate Probe empfohlen
- Secrets aus Dateien/Secret Store
- externe Backups, Monitoring von `/health/ready` und internen Metriken
- Ressourcenlimits und definierte Egress Policy

### Distributed

- Server mit PostgreSQL
- mehrere Probes pro relevanter Region/Netzwerkzone
- Quorum, offline Queue und Zertifikatsrotation
- Browser-Worker separiert

## 13. Security-Release-Gates

Ein Release ist blockiert bei:

- bekanntem kritischem oder hohem Befund ohne genehmigte zeitlich begrenzte Ausnahme
- Mandantentrennung ohne automatisierten Negativtest
- Secret in Log-, API-, Audit-, Export- oder Screenshot-Fixture
- standardmäßig deaktivierter TLS-Prüfung
- Browserworker mit Hostzugriff oder ohne harte Ressourcenlimits
- fehlender AuthZ-Prüfung an einem Schreibendpunkt
- nicht reproduzierbarem Backup/Restore der unterstützten Engines
