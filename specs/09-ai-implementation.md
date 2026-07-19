# 09 – Arbeitsvertrag für AI-Implementierung

## 1. Zweck

Dieses Kapitel macht die AI-Entwicklung überprüfbar. Ein Agent darf Entscheidungen treffen, die innerhalb der Spezifikation liegen. Produktumfang, Sicherheitsgrenzen oder öffentliche Verträge darf er nicht still verändern.

## 2. Rollen

Für jedes Arbeitspaket werden drei logisch getrennte Rollen verwendet. Sie dürfen von verschiedenen Agenten oder in streng getrennten Kontexten ausgeführt werden:

### Builder

Implementiert einen kleinen, klar abgegrenzten Slice und dessen Tests. Er liefert keine Selbstfreigabe.

### Reviewer

Prüft Diff und Verhalten gegen Requirement-IDs, Architektur, Sicherheit und Migrationsregeln. Er sucht aktiv nach fehlenden Negativfällen und darf Tests nicht allein aufgrund des Builders akzeptieren.

### Validator

Startet aus sauberem Checkout, führt die vorgeschriebenen Prüfungen aus, erzeugt Evidence und bestätigt nur reproduzierbare Ergebnisse.

Der Nutzer erhält erst dann ein Artefakt zum manuellen Test, wenn Review und Validation erfolgreich sind.

## 3. Quellen der Wahrheit

Agenten lesen vor einer Änderung mindestens:

1. Root-`AGENTS.md` des späteren Repositories
2. dieses Spec-README und betroffene Kapitel
3. betroffene Maschinenverträge und Abnahmeszenarien
4. vorhandene ADRs und Modul-README
5. vorhandenen Code und Tests im Änderungspfad

Annahmen aus Modellwissen haben niemals Vorrang vor Repository und Verträgen.

## 4. Größe eines Arbeitspakets

Ein Paket soll einen überprüfbaren vertikalen Nutzen liefern und in der Regel:

- höchstens ein primäres Fachverhalten ändern
- höchstens einen öffentlichen Vertrag erweitern
- Datenmigration separat sichtbar machen
- im Diff idealerweise unter 800 handgeschriebenen Zeilen bleiben
- in weniger als 30 Minuten vollständig validierbar sein

Große Features werden durch Vertrag → Domain → Persistenz → API → UI → E2E in aufeinander aufbauende, aber jeweils grüne Pakete zerlegt. Platzhalter-Endpunkte, die Erfolg vortäuschen, sind verboten.

## 5. Pflichtinhalt jedes Issue-Prompts

```text
Titel:
Zielrelease:
Requirement-IDs:
Nutzerergebnis:
In Scope:
Out of Scope:
Betroffene Verträge:
Sicherheits-/Datenrisiken:
Akzeptanzfälle:
Pflichtprüfungen:
Erwartete Evidence:
```

Fehlt eine notwendige Information, liest der Agent die Spec und macht eine konservative, dokumentierte Annahme. Nur eine Entscheidung, die Umfang, öffentliche Semantik, Datenverlust oder Sicherheitsniveau ändert, wird an den Eigentümer eskaliert.

## 6. Implementierungsablauf

1. Aktuellen Stand und uncommitted Änderungen prüfen; fremde Änderungen erhalten.
2. Requirement-IDs und relevante Invarianten notieren.
3. Zuerst einen fehlschlagenden fachlichen/vertraglichen Test hinzufügen.
4. Öffentlichen Vertrag im selben Change aktualisieren, falls nötig.
5. Minimalen vollständigen Pfad implementieren.
6. Betroffene kurze Tests, dann komplette Gates ausführen.
7. Diff auf Secrets, Debugcode, unbeabsichtigte API- und Schemaänderungen prüfen.
8. Reviewer mit Spec, Diff und Testausgaben arbeiten lassen.
9. Reviewpunkte implementieren; niemals bloß wegdiskutieren oder Tests abschwächen.
10. Validator aus sauberem Zustand Evidence erzeugen lassen.
11. Erst danach Paket als fertig markieren.

## 7. Änderungsregeln

### Erlaubt ohne Eigentümerentscheidung

- interne Benennung, kleine Refactorings und Bibliothekswahl innerhalb des festgelegten Stacks
- zusätzliche Tests, Telemetrie und Dokumentation
- sicherere Validierung, wenn gültige Vertragseingaben unverändert bleiben
- Leistungsoptimierung ohne Semantikänderung
- Fehlerbehebung, die klar einer Spec-Invariante folgt

### Benötigt Spec-Änderung und Reviewer

- neues API-Feld, Enum, Endpunkt, Config-Feld oder Proto-Nachricht
- neue Tabelle/Migration
- neue Permission oder Auditwirkung
- Änderung an Retention, Uptime oder Zustandsautomat
- neues Secret oder neuer externer Datenfluss

### Benötigt Eigentümerentscheidung

- Feature außerhalb der Roadmap
- Breaking API-/Config-/Proto-Änderung
- Abschwächung einer Sicherheitskontrolle
- Datenverlust-/Migrationsrisiko ohne sicheren Pfad
- neue externe Infrastrukturpflicht, Cloudabhängigkeit oder Lizenz mit Copyleftwirkung
- Änderung von Lizenz, Marke oder Geschäftsmodell

## 8. Anti-Cheating-Regeln

Ein Agent DARF NICHT:

- Test überspringen, löschen, lockern oder mit einem konstanten Mock „grün“ machen, um die Implementierung zu bestehen
- Snapshots blind aktualisieren
- Fehler schlucken oder immer Erfolg zurückgeben
- `sleep` als Synchronisationsersatz in Tests verwenden, wenn kontrollierte Uhr/Events möglich sind
- reale Netzwerkdienste oder zufällige Internetendpunkte in CI voraussetzen
- ein Secret in Testdaten einchecken
- `unsafe`, `unwrap`, `expect`, `panic!`, `todo!` oder `unimplemented!` in einem produktiven Pfad ohne genehmigte Ausnahme einführen
- Datenbankfehler zu `TARGET_FAILURE` konvertieren
- neue Abhängigkeit hinzufügen, ohne Lizenz, Wartungszustand und Notwendigkeit zu prüfen
- bestehende fremde Änderungen überschreiben oder den Worktree destruktiv bereinigen
- „vollständig“ melden, solange Pflichtprüfung rot, übersprungen oder nicht ausgeführt ist

Ein nicht ausführbarer Test wird als Blocker mit Ursache gemeldet, nicht als bestanden.

## 9. Coding-Regeln

### Rust

- `#![forbid(unsafe_code)]` in eigenen Crates
- typisierte IDs statt roher Strings an Domänengrenzen
- `thiserror`-artige typisierte Fehler in Bibliotheken; Kontext an Prozessgrenzen
- keine Panics bei Nutzerinput oder externen Fehlern
- keine blockierende I/O auf Tokio-Workerthreads
- explizite Timeouts und Cancellation
- injizierbare Clock und ID-Quelle in fachlicher Logik
- öffentliche Funktionen dokumentiert; interne Abstraktion nur bei realer Wiederholung

### SQL

- gebundene Parameter und explizite Spalten
- keine `SELECT *` in produktiven Queries
- Query-Anzahl und Plan für Listen/Hot Paths testen
- Mandantenschlüssel in Constraints und Indizes berücksichtigen
- Migrationen niemals nach Veröffentlichung ändern

### TypeScript/React

- `strict`, keine ungeprüften `any`
- API-Typen aus OpenAPI generieren; keine manuell driftenden Duplikate
- Serverzustand nicht in globalem UI-Store duplizieren
- Komponenten enthalten keine Berechtigungslogik als Sicherheitsgrenze
- jeder asynchrone Screen hat Loading, Empty, Error und Success
- Accessibility ist Teil des Tests, kein spätes Styling

## 10. Abhängigkeitsregeln

Vor einer neuen Dependency dokumentiert der Agent:

- welches Problem sie löst
- warum Standardbibliothek/vorhandene Dependency nicht genügt
- Lizenz und aktive Wartung
- Auswirkungen auf Binary-/Bundlegröße und Supply Chain
- Alternative, falls Projekt unmaintained wird

Git-Dependencies und unversionierte Container-Tags sind in Releases verboten.

## 11. Evidence-Format pro Arbeitspaket

```markdown
## Implementation Evidence

- Commit: `<sha>`
- Requirements: `PRD-...`
- Contracts changed: yes/no + files
- Migrations: none / IDs
- Tests added: list
- Commands executed: exact commands and exit codes
- Security review: findings/none
- Known limitations: list/none
- Reviewer verdict: approved/changes requested
- Validator verdict: passed/failed
```

Keine gekürzten Aussagen wie „alle Tests bestanden“ ohne Befehle und Exit Codes.

## 12. Repository-Gates

- Protected main branch; nur geprüfte Pull Requests
- erforderliche Statuschecks können nicht durch den Builder deaktiviert werden
- CODEOWNERS oder separate Reviewrolle für `contracts/`, `migrations/`, Auth, Secrets und Probe-Protokoll
- Renovate/Dependabot nur als Pull Requests mit vollständiger CI
- Release-Tags werden durch Pipeline aus einem geprüften Commit erstellt, nie aus lokalem Zustand
- generierte Artefakte erhalten Provenance und sind mit dem Quellcommit verknüpft

## 13. Bootstrap-Auftrag für den ersten Agenten

Der erste Implementierungsauftrag lautet bewusst nur:

> Erzeuge das Takt-Monorepo gemäß `01-architecture.md`, pinne Toolchains, richte reproduzierbare Format-/Lint-/Test-Gates ein, übernimm die Verträge unverändert, erzeuge API-/Proto-Typen und implementiere ausschließlich Liveness/Readiness plus einen minimalen Domain-Test. Keine Monitorfunktion, keine Auth, keine Datenbanktabellen. Liefere Evidence nach Kapitel 11.

Der zweite Auftrag führt die erste Migration, Default-Organisation/-Projekt und lokalen Admin ein. Der dritte baut Monitor CRUD. Erst der vierte implementiert den vollständigen HTTP-Checkpfad. So wird verhindert, dass ein einzelner großer Prompt ein scheinbar komplettes, aber unprüfbares System erzeugt.

## 14. Abnahmeprompt für einen unabhängigen Agenten

```text
Prüfe den vorliegenden Change als unabhängiger Takt-Validator.
Lies README, betroffene Requirements, Verträge und Acceptance-Szenarien.
Vertraue keiner Selbstaussage im Change. Führe die vorgeschriebenen Prüfungen
aus einem sauberen Checkout aus. Suche besonders nach Vertragsdrift,
Autorisierungslücken, Secret-Leaks, falscher Fehlerklassifikation,
nicht-idempotentem Verhalten und abgeschwächten Tests. Ändere nichts.
Berichte zuerst Release-Blocker mit Datei/Zeile und reproduzierbarem Nachweis,
dann ausgeführte Befehle, danach verbleibende Risiken. Wenn eine Prüfung nicht
ausführbar ist, markiere sie als nicht verifiziert, niemals als bestanden.
```

## 15. Übergabe an den Eigentümer

Der Eigentümer bekommt pro Release:

- installierbare signierte Artefakte
- kurze Installations-/Upgradeanleitung
- Smoke-Testliste aus `07-quality-acceptance.md`
- Evidence Bundle
- bekannte Einschränkungen in klarer Sprache

Interne Agentendiskussionen, temporäre Skripte und ungeprüfte Builds gehören nicht zur Übergabe.
