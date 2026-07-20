# Verfahren zur Implementierungsnachverfolgung

Dieses Verzeichnis ist die operative Sicht auf die verbindliche Spezifikation. Es ersetzt keine Spec und ändert keine Produktsemantik. Es beantwortet stattdessen getrennt:

1. Was fordert die Spec?
2. Wie viel davon ist im aktuellen Code tatsächlich vorhanden?
3. Wie stark ist dieser Stand belegt?
4. Welche Spec-, Implementierungs- oder Evidence-Probleme sind bekannt?
5. Welches kleinste überprüfbare Paket ist als Nächstes ausführbar?

## Verbindliche Register

| Datei | Rolle |
|---|---|
| `requirements.yaml` | Genau ein Eintrag pro kanonischer `PRD-*`-ID mit Coverage, Verification, Spec Health und konkreter Evidence |
| `work-packages.yaml` | Abhängiger, release-geordneter Backlog aus vertikalen und separat prüfbaren Paketen |
| `findings.yaml` | Offene und gelöste Spec-Fehler, Widersprüche, Lücken, Entscheidungen und Evidence-Defizite |
| `current-status.md` | Lesbare, datierte Momentaufnahme; die YAML-Register bleiben die strukturierte Quelle |

Die Spezifikationsrangfolge aus `specs/README.md` bleibt unverändert. Bei einem Widerspruch wird ein Finding eröffnet und das betroffene Paket blockiert; der Tracker darf keinen niedrigeren Vertrag still bevorzugen.

## Zwei unabhängige Achsen statt eines irreführenden Prozentsatzes

Requirement Coverage beschreibt Produktverhalten:

| Wert | Bedeutung |
|---|---|
| `none` | Kein Verhalten der Requirement ist implementiert. Vorbereitende Dateien oder ein Schema allein zählen nicht. |
| `partial` | Mindestens ein normativer Teil ist implementiert, aber mindestens ein anderer fehlt. Der fehlende Teil steht im `note`. |
| `full` | Alle normativen Teile der Requirement sind im aktuellen Source-Stand vorhanden. Das ist noch keine Freigabe. |

Verification beschreibt unabhängig die Belegstärke:

| Wert | Mindestbedeutung |
|---|---|
| `none` | Kein verwertbarer Nachweis |
| `evidence_only` | Vorhandene historische Evidence wurde geprüft, aber nicht gegen den aktuellen Stand wiederholt |
| `focused_local` | Betroffene Tests/Gates liefen lokal auf dem referenzierten Stand |
| `full_local` | Alle Repository-Gates liefen lokal, ohne Skip |
| `independent` | Ein unabhängiger Validator bestätigte einen sauberen Checkout |
| `ci` | Derselbe Commit bestand die geschützte CI |
| `release` | Release-Exit-Kriterien und unveränderliches Evidence Bundle sind vollständig |

Damit kann zum Beispiel ein Vertrag `partial` und zugleich `focused_local` sein. „Test grün“ macht eine zusammengesetzte Requirement nicht automatisch vollständig; „Code vorhanden“ macht sie nicht verifiziert.

## Work-Package-Zustände

| Zustand | Eintrittskriterium |
|---|---|
| `planned` | Scope, Requirements, Abhängigkeiten, Acceptance und erwartete Evidence sind definiert. |
| `in_progress` | Genau dieses Paket wird umgesetzt; Requirement- und Risikoanalyse liegt vor. |
| `implemented` | Builder-Verhalten und fokussierte Checks sind vorhanden, unabhängige Freigabe fehlt aber noch. |
| `verified` | Reviewer und Validator haben den commit-gebundenen Change akzeptiert; alle Paketgates sind grün. |
| `blocked` | Eine konkret referenzierte Spec-/Owner-Entscheidung verhindert sinnvolle Umsetzung. |

Ein Paket wechselt nie direkt von `planned` zu `verified`. Ein rotes, nicht ausgeführtes oder übersprungenes Gate verhindert `verified`.

## Ablauf pro Bestandsaufnahme

1. Ausgangspunkt festhalten: `git status --short`, vollständige Commit-SHA, Spec-Version und Datum. Fremde Änderungen bleiben unangetastet.
2. Kanonische IDs ausschließlich aus `specs/00-product-requirements.md` lesen. `pnpm check:tracking` prüft Vollständigkeit und unbekannte IDs.
3. Pro Requirement Contracts, Acceptance, Code, Tests und bestehende Evidence abgleichen. Coverage und Verification separat aktualisieren; jede Teilbehauptung erhält Pfade und eine Restbeschreibung.
4. Neue Probleme zuerst in `findings.yaml` erfassen. Keine Implementierung starten, wenn ein höher priorisierter Vertrag fehlt oder widerspricht.
5. Work-Package-DAG prüfen und das kleinste unblocked Paket des frühesten offenen Releases wählen. Voraussetzungen müssen mindestens `implemented`, für sicherheits-/migrationskritische Grenzen grundsätzlich `verified` sein.
6. Issue/Prompt aus dem Paket erzeugen und unter 800 handgeschriebenen Zeilen beziehungsweise unter 30 Minuten Validierungszeit halten. Wird das unrealistisch, das Paket vor Implementierung teilen.
7. Test-first umsetzen, Contract und Migration im selben Change aktualisieren, fokussierte Checks und dann alle vorgeschriebenen Gates ausführen.
8. Builder, Reviewer und Validator bleiben logisch getrennt. Der Validator arbeitet auf einem sauberen, commit-gebundenen Checkout.
9. Im selben Change Evidence, Requirement-Ledger, Paketstatus und neue/gelöste Findings aktualisieren. Historische fehlgeschlagene Evidence wird ergänzt, nicht überschrieben.
10. Release-Status wird nur aus Exit-Kriterien und Release Evidence bestimmt, nie aus der Anzahl vorhandener Dateien oder geschlossener Issues.

`AGENTS.md` macht diese Auswahl für allgemeine Fortsetzungsaufträge verbindlich.
Ein Prompt wie „Fahre mit der nächsten offenen Aufgabe fort“ genügt: Der Agent
wählt das nächste ausführbare Paket deterministisch nach Release, Blockern und
Dateireihenfolge, arbeitet genau dieses Paket ab und pflegt Register und Evidence
im selben Change. Eine Rückfrage ist nur bei einer echten Owner-Entscheidung oder
einem nicht innerhalb des Scopes lösbaren Blocker vorgesehen.

## Finding-Triage

`decision` legt fest, wie weitergearbeitet wird:

- `implementation`: Spec ist ausreichend; ein Code-/Test-/Evidence-Paket kann das Problem lösen.
- `spec_change`: Öffentliche Semantik, Requirement-ID oder Vertragsklarheit fehlt. Zuerst ein kleiner Spec-Change mit Review und Test, dann Implementierung.
- `owner_decision`: Lizenz, Marke, Sicherheitsabschwächung, externer Dienst, Datenrisiko oder Scope benötigt eine explizite Eigentümerentscheidung.

Severity beschreibt die Auswirkung auf den nächsten Release, nicht die Schwierigkeit. Ein Finding wird nur `resolved`, wenn die Resolution im Repository nachweisbar ist. Eine bewusst tolerierte Abweichung wird `accepted` und braucht Begründung, Ablaufdatum und kompensierende Maßnahme in der Resolution.

## Pflichtinhalt eines Paket-Issues

Der Prompt aus Kapitel 09 wird um Tracking-Metadaten ergänzt:

```text
Package-ID:
Titel:
Zielrelease:
Requirement-IDs:
Finding-IDs:
Abhängigkeiten:
Nutzerergebnis:
In Scope:
Out of Scope:
Betroffene Verträge:
Sicherheits-/Datenrisiken:
Akzeptanzfälle:
Pflichtprüfungen:
Erwartete Evidence:
Ledger-Änderung bei Abschluss:
```

Definition of Ready:

- alle IDs sind kanonisch und `pnpm check:tracking` ist grün
- betroffene Contracts/Acceptance-Szenarien sind gelesen und nicht widersprüchlich
- Blocker-/Owner-Entscheidungen sind gelöst
- Acceptance ist als beobachtbares Verhalten formuliert
- Datenmigration, AuthZ, Audit, Secrets, Observability und beide Engines sind ausdrücklich bewertet

Definition of Done:

- alle Paket-Acceptance-Punkte besitzen positive und relevante negative Tests
- öffentliche Verträge und Migrationen sind im selben Change enthalten
- fokussierte und vollständige Gates sind mit Exit Codes dokumentiert
- Review- und Validator-Verdict sind commit-gebunden
- `requirements.yaml`, `work-packages.yaml`, `findings.yaml` und Implementation Evidence sind aktualisiert
- bekannte Einschränkungen stehen explizit im Evidence-Dokument

## Automatischer Gate

```text
pnpm check:tracking
pnpm test:tools
```

Der Gate prüft derzeit:

- alle 54 kanonischen Requirements genau einmal im Ledger
- nur erlaubte Coverage-/Verification-/Finding-/Paketstatuswerte
- Evidence für jede nicht leere Coverage
- vollständige Requirement-Zuordnung zu mindestens einem Arbeitspaket
- existierende Paketabhängigkeiten und einen azyklischen DAG
- gültige Finding-Referenzen und existierende referenzierte Dateien
- keine unbekannten `PRD-*`-IDs in Code, Migrationen, Tests und Evidence, außer eng pfadgebundenen offenen Ausnahmen

Eine Ausnahme ist kein gültiger Requirement-Ersatz. Sie verhindert lediglich, dass ein bereits dokumentierter Fehler neue unbekannte Verweise verdeckt; das zugehörige Finding bleibt offen.

## Pflegefrequenz

- bei jedem Work-Package-PR: betroffene Ledger-Zeilen, Paketstatus, Findings und Evidence
- wöchentlich oder vor Backlog-Planung: Baseline-Commit und Abhängigkeits-DAG auditieren
- vor jedem Release Candidate: vollständige Neuberechnung aus sauberem Checkout, keine `evidence_only`-Einstufung für Release-Muss-Anforderungen
- nach jeder Spec-Änderung: Gate laufen lassen und Paketzuordnung/Acceptance anpassen, bevor Feature-Code folgt
