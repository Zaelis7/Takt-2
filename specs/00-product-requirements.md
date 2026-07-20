# 00 – Produktanforderungen

## 1. Vision

Takt macht zuverlässiges Uptime-Monitoring so einfach wie ein kleines Self-Hosting-Tool, ohne Automatisierung, Datenhaltung und verteilten Betrieb zu nachträglichen Sonderfällen zu machen.

Der zentrale Produktvorteil lautet:

> **In Minuten startklar, vollständig automatisierbar und ohne Architekturwechsel bis zum professionellen Betrieb skalierbar.**

## 2. Problemstellung

Bestehende einfache Monitoring-Tools sind angenehm zu installieren, zeigen aber typischerweise mindestens eine dieser Grenzen:

- Verwaltungsfunktionen sind nur über die UI oder eine interne Socket-Schnittstelle erreichbar.
- Infrastruktur kann nicht zuverlässig deklarativ verwaltet werden.
- Datenbankunterstützung ist an einzelne Engines oder nicht produktionsgerechte Migrationspfade gebunden.
- Remote-Standorte, Quorum und Offline-Pufferung fehlen oder sind nachträglich angefügt.
- Alerting, Wartungsfenster und Incidents besitzen keine klaren Zustandsmodelle.
- Große, stark gekoppelte Serverdateien und dynamisch typisierte Monitoroptionen erschweren sichere Änderungen.
- Plugins, Benachrichtigungen und Spezialchecks wachsen schneller als stabile Verträge und Tests.

Takt löst diese Punkte durch stabile öffentliche Verträge, explizite Zustände und eine von Anfang an getrennte Ausführungs-, Bewertungs- und Benachrichtigungspipeline.

## 3. Zielgruppen

### P1 – Self-Hoster

Betreibt 5 bis 100 Monitore auf einem einzelnen Host. Erwartet eine einfache Installation, SQLite, eine klare UI und wenige Pflichtfelder.

### P2 – Plattformteam

Verwaltet 100 bis 10.000 Monitore automatisiert. Erwartet PostgreSQL, API/CLI, GitOps, OIDC, RBAC, Audit-Log, Wartungsfenster und verteilte Probes.

### P3 – MSP oder internes Operations-Team

Trennt Kunden oder Bereiche nach Organisation und Projekt. Erwartet rollenbasierte Sichtbarkeit, Statusseiten, Incidents, Abonnenten und belastbare Upgrades.

### P4 – Entwickler

Legt Monitore aus CI/CD, Terraform oder Kubernetes-Discovery an. Erwartet Idempotenz, Versionierung, maschinenlesbare Fehler und vorhersehbare Änderungen.

## 4. Produktprinzipien

1. **API und UI sind gleichberechtigte Clients.** Die UI DARF keine privilegierte interne Verwaltungslogik verwenden.
2. **Einfacher Start, expliziter Ausbau.** SQLite und lokale Prüfung bleiben möglich; Produktion erhält PostgreSQL und Probes ohne Neuimplementierung.
3. **Keine falschen Ausfälle.** Interne Fehler, abgelaufene Leases oder Datenbankprobleme DÜRFEN NICHT als Zielausfall gemeldet werden.
4. **Deklarativ ohne Überraschungen.** Änderungen werden geplant; Löschungen sind nur mit `--prune` und klarer Herkunft erlaubt.
5. **Sichere Voreinstellungen.** Geheimnisse, Tokens, Sessions und Probe-Verbindungen sind standardmäßig geschützt.
6. **Erklärbare Zustände.** Jede Zustandsänderung lässt sich auf Messungen, Regeln und Zeitpunkte zurückführen.
7. **Erweiterbarkeit an Grenzen.** Neue Checks und Integrationen verwenden typisierte Verträge statt ungeprüfter Optionsobjekte.

## 5. Kernbegriffe

- **Monitor:** Gewünschte Überwachung eines Ziels mit Zeitplan und Auswertungsregel.
- **Check:** Einzelne geplante Ausführung eines Monitors.
- **Observation:** Rohes Ergebnis eines Checks.
- **Evaluation:** Fachliche Einordnung einer Observation, etwa `UP` oder `DOWN`.
- **Probe:** Prozess, der Checks an einem Standort ausführt.
- **Incident:** Nutzerorientiertes Ereignis auf einer Statusseite.
- **Alert:** Interner Benachrichtigungszustand eines Monitors.
- **Silence:** Zeitlich begrenzte Unterdrückung von Benachrichtigungen.
- **Maintenance:** Geplante Zeitspanne, in der Ergebnisse besonders behandelt werden.
- **Project:** Sicherheits- und Verwaltungsgrenze innerhalb einer Organisation.

## 6. Funktionale Anforderungen

### 6.1 Monitore und Checks

- **PRD-MON-001:** Ein Nutzer MUSS Monitore über UI, REST API, CLI und deklarative Dateien anlegen, lesen, ändern, pausieren und löschen können.
- **PRD-MON-002:** 0.1 MUSS HTTP(S), TCP, DNS, ICMP, TLS-Zertifikat und Push-Heartbeat unterstützen.
- **PRD-MON-003:** Ein Monitor MUSS einen stabilen UUID-Identifier und einen innerhalb seines Projekts eindeutigen, nutzerdefinierten `slug` besitzen.
- **PRD-MON-004:** Zeitpläne MÜSSEN Intervalle von 10 Sekunden bis 24 Stunden unterstützen. Werte unter 30 Sekunden MÜSSEN als ressourcenintensiv gekennzeichnet werden.
- **PRD-MON-005:** Ein Nutzer MUSS einen Check sofort auslösen und dessen Ergebnis anhand einer Check-ID verfolgen können.
- **PRD-MON-006:** Änderungen an Prüfkriterien MÜSSEN revisioniert und im Audit-Log nachvollziehbar sein.
- **PRD-MON-007:** Ab 0.2 MUSS ein Monitor einem oder mehreren Probe-Standorten zugeordnet und per `any`, `all` oder `quorum` ausgewertet werden können.
- **PRD-MON-008:** Browserchecks ab 0.3 MÜSSEN außerhalb des Serverprozesses in einer eingeschränkten Laufzeit ausgeführt werden.

### 6.2 API und Automatisierung

- **PRD-API-001:** Jede Funktion zur Verwaltung von 0.1-Muss-Ressourcen MUSS über `/api/v1` verfügbar sein.
- **PRD-API-002:** Die API MUSS OpenAPI 3.1 dokumentieren und in CI auf Rückwärtskompatibilität geprüft werden.
- **PRD-API-003:** Schreiboperationen MÜSSEN Idempotency Keys unterstützen; Änderungen MÜSSEN optional per ETag vor verlorenen Updates schützen.
- **PRD-API-004:** Listen MÜSSEN Cursor-Pagination, Filter und stabile Sortierung unterstützen.
- **PRD-API-005:** Fehler MÜSSEN dem Problem-Details-Format mit stabilem Fehlercode folgen.
- **PRD-AUT-001:** `taktctl validate`, `plan`, `apply` und `export` MÜSSEN in 0.1 verfügbar sein.
- **PRD-AUT-002:** `apply` DARF nicht verwaltete Ressourcen nicht verändern.
- **PRD-AUT-003:** `--prune` DARF nur Ressourcen löschen, die zuvor von derselben deklarativen Quelle verwaltet wurden.
- **PRD-AUT-004:** 0.3 MUSS einen Terraform Provider für Organisationen, Projekte, Monitore, Statusseiten und Wartungsfenster liefern.
- **PRD-AUT-005:** 0.3 MUSS Kubernetes Services und Ingresses anhand opt-in Labels entdecken können.

### 6.3 Zustände und Alerting

- **PRD-ALT-001:** Monitorzustände MÜSSEN mindestens `PENDING`, `UP`, `DEGRADED`, `DOWN`, `PAUSED`, `MAINTENANCE`, `UNKNOWN` unterscheiden.
- **PRD-ALT-002:** Takt MUSS Wiederholungen vor einem Wechsel zu `DOWN` und erfolgreiche Wiederholungen vor `UP` konfigurierbar machen.
- **PRD-ALT-003:** Benachrichtigungen MÜSSEN über einen transaktionalen Outbox-Prozess vom Speichern der Auswertung entkoppelt sein.
- **PRD-ALT-004:** Der Versand MUSS mindestens einmal erfolgen und anhand stabiler Event-IDs deduplizierbar sein. „Exactly once“ DARF NICHT versprochen werden.
- **PRD-ALT-005:** 0.2 MUSS Acknowledgements, Silences, Wartungsfenster und Flapping-Erkennung unterstützen.
- **PRD-ALT-006:** Ein interner Systemfehler MUSS `UNKNOWN` oder einen separaten Systemzustand erzeugen und DARF NICHT allein einen Ausfallalarm auslösen.

### 6.4 Benachrichtigungen

- **PRD-NOT-001:** 0.1 MUSS Webhook, SMTP-E-Mail, Slack, Discord und Telegram unterstützen.
- **PRD-NOT-002:** Ziel- und Zugangsdaten MÜSSEN getrennt von Monitoren verwaltet, verschlüsselt gespeichert und in Antworten maskiert werden.
- **PRD-NOT-003:** Nutzer MÜSSEN Benachrichtigungskanäle testen können, ohne einen künstlichen Monitor-Ausfall zu erzeugen.
- **PRD-NOT-004:** Zustellung MUSS Retry mit exponentiellem Backoff, Dead-Letter-Zustand und sichtbarer Fehlerursache besitzen.
- **PRD-NOT-005:** Kanal-Plugins außerhalb des Kernumfangs SOLLEN erst nach einem stabilen Pluginvertrag nach 0.3 aufgenommen werden.

### 6.5 Statusseiten und Incidents

- **PRD-STA-001:** Nutzer MÜSSEN öffentliche oder token-geschützte Statusseiten mit Gruppen und ausgewählten Monitoren erstellen können.
- **PRD-STA-002:** Statusseiten MÜSSEN aktuelle Zustände, 24-Stunden-Historie, konfigurierbare Uptime-Zeiträume und aktive Wartungen anzeigen.
- **PRD-STA-003:** 0.2 MUSS manuelle Incidents mit zeitlicher Abfolge von Updates und Auswirkungen unterstützen.
- **PRD-STA-004:** Automatische Monitorzustände und redaktionelle Incidents MÜSSEN getrennt bleiben, können aber miteinander verknüpft werden.
- **PRD-STA-005:** 0.3 MUSS E-Mail-Abonnenten mit Double-Opt-in, Abmeldung und minimaler Datenspeicherung unterstützen.
- **PRD-STA-006:** Öffentliche Statusseiten DÜRFEN keine internen URLs, Probe-IDs, Fehlermeldungen oder Metadaten offenlegen, sofern sie nicht explizit freigegeben wurden.

### 6.6 Identität und Berechtigungen

- **PRD-IAM-001:** 0.1 MUSS einen lokalen Administrator, sichere Sitzungen und API-Tokens mit Scopes unterstützen.
- **PRD-IAM-002:** 0.3 MUSS OIDC Authorization Code Flow mit PKCE unterstützen.
- **PRD-IAM-003:** 0.3 MUSS Organisationen, Projekte und die Rollen `owner`, `admin`, `editor`, `operator`, `viewer` unterstützen.
- **PRD-IAM-004:** Rechte MÜSSEN serverseitig auf jede Ressourcenoperation angewendet werden; ausgeblendete UI-Elemente genügen nicht.
- **PRD-IAM-005:** Sicherheitsrelevante und schreibende Aktionen MÜSSEN in einem manipulationserschwerenden Audit-Log erscheinen.

### 6.7 Datenhaltung

- **PRD-DATA-001:** Fachliche Repository-Semantik MUSS auf PostgreSQL 16+ und SQLite gleich sein. Engine-spezifische Einschränkungen MÜSSEN als explizite, getestete Kapazitätsgrenzen dokumentiert werden und DÜRFEN das fachliche Ergebnis nicht still verändern.
- **PRD-DATA-002:** Schemaänderungen MÜSSEN als vorwärtsgerichtete, nummerierte Migrationen ausgeliefert werden, nach Veröffentlichung unveränderlich bleiben und transaktional laufen, sofern die Engine dies erlaubt. Ein unbekanntes neueres Schema MUSS den Start ablehnen; während einer Migration MUSS Readiness fehlschlagen.
- **PRD-DATA-004:** Persistente öffentliche Entitäten MÜSSEN unveränderliche UUIDv7-IDs verwenden. Zeitpunkte MÜSSEN als UTC mit Mikrosekundenpräzision gespeichert werden; jede änderbare Ressource MUSS `created_at`, `updated_at` und eine monoton steigende `version` besitzen.

### 6.8 Importmigration

- **PRD-MIG-001:** 0.2 MUSS einen Importer für einen dokumentierten, unterstützten Uptime-Kuma-Datenexport bereitstellen.
- **PRD-MIG-002:** Der Import MUSS als `analyze`, `plan` und `apply` ablaufen und nicht unterstützte Felder transparent melden.
- **PRD-MIG-003:** Passwörter oder unentschlüsselbare Geheimnisse DÜRFEN NICHT vorgetäuscht importiert werden; sie sind als erforderliche Nacharbeit zu markieren.
- **PRD-MIG-004:** Der Import MUSS wiederholbar sein und bereits importierte Ressourcen anhand stabiler Herkunft aktualisieren statt duplizieren.

## 7. Nichtfunktionale Anforderungen

- **PRD-NFR-001:** Ein frisches lokales System MUSS mit einem Befehl und ohne externe Datenbank starten können.
- **PRD-NFR-002:** Produktionsbetrieb MUSS PostgreSQL 16 oder neuer unterstützen.
- **PRD-NFR-003:** Server und Probe MÜSSEN als Container und statisch möglichst eigenständige Linux-Binaries für `amd64` und `arm64` veröffentlicht werden.
- **PRD-NFR-004:** Die Standardinstallation MUSS ohne Telemetrie zu einem Drittanbieter funktionieren. Optionale Nutzungsmetriken erfordern Opt-in.
- **PRD-NFR-005:** Alle Zeiten werden intern als UTC gespeichert; Anzeige verwendet die ausgewählte IANA-Zeitzone.
- **PRD-NFR-006:** Deutsch und Englisch MÜSSEN ab 0.1 vollständig unterstützt werden. Fehlende Übersetzungen DÜRFEN die UI nicht beschädigen.
- **PRD-NFR-007:** Ein Server-Upgrade MUSS vor Migration ein Backup prüfen oder eine explizite Bestätigung für dessen Auslassung verlangen.
- **PRD-NFR-008:** Beobachtbarkeit MUSS strukturierte Logs, Prometheus-Metriken, Health- und Readiness-Endpunkte sowie OpenTelemetry-Traces ermöglichen.
- **PRD-NFR-009:** Barrierefreiheit der Verwaltungsoberfläche SOLL WCAG 2.2 AA erfüllen; Tastaturbedienung und sichtbare Fokusführung sind Muss-Kriterien.
- **PRD-NFR-010:** Die Kernlogik MUSS deterministisch mit einer injizierbaren Uhr und kontrollierbarer Zufallsquelle testbar sein.

## 8. Bewusste Nicht-Ziele bis 0.3

- Vollständige APM-, Log- oder Infrastruktur-Telemetrieplattform
- Beliebige Shell-Befehle im Serverprozess
- Gleichwertige Unterstützung aller SQL-Datenbanken
- Multi-Region-HA des Control Planes
- Message Broker oder frei skalierende Microservice-Landschaft
- Kubernetes Operator vor Stabilisierung von API und Terraform Provider
- Marketplace mit ungeprüftem Drittcode
- Nachbau jedes existierenden Benachrichtigungskanals
- Native Mobile Apps

## 9. Erfolgsmessung

- Ein neuer Nutzer erstellt den ersten HTTP-Monitor in unter fünf Minuten.
- Ein Plattformteam kann 500 Monitore deklarativ anwenden, erneut anwenden und ohne Drift planen.
- Kein getesteter interner Ausfallpfad erzeugt einen falschen `DOWN`-Alarm.
- 95 % der typischen Verwaltungsaufgaben sind über UI, API und CLI möglich.
- Eine 0.1-Installation kann ohne Datenverlust auf 0.3 aktualisiert werden.
