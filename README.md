# Takt – Spezifikationspaket bis Version 0.3

Status: **Implementierungsfreigabe**  
Spec-Version: **1.0.0**  
Zielrelease: **Takt 0.3.0**  
Stand: **19. Juli 2026**

Takt ist eine selbst hostbare Monitoring- und Statusseiten-Plattform. Sie verbindet die einfache Bedienung eines Home-Lab-Tools mit einer stabilen API, deklarativer Konfiguration, produktionsfähiger PostgreSQL-Anbindung und verteilten Probes.

Dieses Verzeichnis ist die verbindliche Spezifikation für eine vollständig AI-gestützte Implementierung. Es beschreibt das Produkt bis einschließlich Version 0.3.0 und ist so aufgebaut, dass ein Implementierungsagent Arbeitspakete ohne zusätzliche Produktentscheidungen erstellen kann.

## 1. Dokumente

| Datei | Zweck |
|---|---|
| `00-product-requirements.md` | Produktziel, Personas, Umfang und Anforderungen |
| `01-architecture.md` | Systemarchitektur, Komponenten und technische Leitplanken |
| `02-domain-data.md` | Domänenmodell, Zustände, Speicherung und Aufbewahrung |
| `03-api-and-automation.md` | Regeln für REST API, CLI, GitOps und Ereignisse |
| `04-probes-and-checks.md` | Check-Typen, Scheduler und Remote-Probe-Protokoll |
| `05-ui-ux.md` | Bedienkonzept und verbindliche Oberflächen |
| `06-security-operations.md` | Sicherheitsmodell, Betrieb, Backups und Upgrades |
| `07-quality-acceptance.md` | Teststrategie, Qualitätsziele und Freigaberegeln |
| `08-roadmap.md` | Lieferumfang und Exit-Kriterien für 0.1, 0.2 und 0.3 |
| `09-ai-implementation.md` | Arbeitsvertrag für Implementierungs- und Review-Agenten |
| `10-traceability.md` | Zuordnung der Anforderungen zu Releases und Abnahme |
| `AGENTS.template.md` | Startvorlage für verbindliche Repository-Anweisungen |
| `contracts/openapi.yaml` | Maschinenlesbarer HTTP-API-Vertrag |
| `contracts/takt-config.schema.json` | Schema für deklarative Konfiguration |
| `contracts/probe.proto` | Maschinenlesbarer Vertrag zwischen Server und Probe |
| `acceptance/*.feature` | Ausführbare Produktabnahme in Gherkin |
| `examples/takt.yaml` | Beispiel für eine deklarative Installation |

## 2. Verbindlichkeit

Die Schlüsselwörter **MUSS**, **DARF NICHT**, **SOLL**, **SOLL NICHT** und **KANN** sind normativ.

Bei Widersprüchen gilt diese Reihenfolge:

1. Maschinenlesbare Verträge in `contracts/`
2. Abnahmeszenarien in `acceptance/`
3. Versionsumfang und Exit-Kriterien in `08-roadmap.md`
4. Fachliche Kapitel `00` bis `10`
5. Beispielkonfigurationen

Ein Widerspruch DARF NICHT stillschweigend interpretiert werden. Er MUSS als Spec-Änderung mit Entscheidung und Test aufgelöst werden.

## 3. Produktentscheidungen

Diese Entscheidungen gelten für 0.3.0:

- **Kern:** Rust, Tokio, Axum/Tower, SQLx, rustls.
- **Weboberfläche:** React und TypeScript; als statische Assets in das Server-Artefakt eingebettet.
- **Architektur:** modularer Monolith plus separater Probe-Prozess; kein Message Broker.
- **Datenbanken:** PostgreSQL für Produktion; SQLite nur für Einzelinstanzen und Evaluation.
- **Schnittstellen:** REST/JSON unter `/api/v1`, OpenAPI 3.1, Server-Sent Events für Live-Aktualisierungen, gRPC über mTLS für Remote-Probes.
- **Betriebsmodell:** self-hosted first; Linux `amd64` und `arm64`; Container und einzelnes Binary.
- **Automatisierung:** vollständige Verwaltungs-API, `taktctl` und deklaratives `plan/apply`.
- **Namenssprache:** API, Datenbankfelder und Quellcode auf Englisch; UI und Dokumentation mindestens Deutsch und Englisch.
- **Lizenzannahme:** Apache-2.0. Vor einer öffentlichen Veröffentlichung MUSS der Projekteigentümer Lizenz, Markenlage und Paketnamen bestätigen.

## 4. Nicht blockierende, später zu bestätigende Entscheidungen

Die Implementierung darf mit den oben genannten Vorgaben beginnen. Vor einem öffentlichen 0.1-Release sind lediglich folgende Eigentümerentscheidungen nötig:

- Ist Apache-2.0 das gewünschte Lizenzmodell oder soll ein Open-Core-Modell vorbereitet werden?
- Sind Name, Domains, Container-Registry und Paketnamen für „Takt“ rechtlich und praktisch verfügbar?
- Soll nach 0.3 eine gehostete Mehrmandanten-Variante angeboten werden? 0.3 baut bereits saubere Organisationsgrenzen, bleibt aber self-hosted.
- Welche Kanäle sollen offiziell für Security-Meldungen und Release-Signaturen verwendet werden?

## 5. Definition des Zielzustands

Takt 0.3.0 ist erreicht, wenn alle Muss-Anforderungen der Releases 0.1 bis 0.3 implementiert sind, alle automatisierten Abnahmeszenarien erfolgreich laufen, keine offenen kritischen oder hohen Sicherheitsbefunde bestehen und ein Upgrade aus jeder vorherigen Minor-Version mit dokumentiertem Backup- und Restore-Test funktioniert.

„Funktioniert auf dem Rechner des Implementierungsagenten“ ist kein Abnahmekriterium. Jede Behauptung MUSS durch reproduzierbare Befehle, Testberichte oder Artefakte belegt sein.
