# 05 – UI- und UX-Spezifikation

## 1. UX-Ziel

Die Oberfläche soll für kleine Installationen leicht wirken und zusätzliche Fähigkeiten erst dann zeigen, wenn sie relevant werden. Alle Aktionen verwenden dieselbe öffentliche API wie externe Clients.

## 2. Informationsarchitektur

### Globale Navigation

- Organisations-/Projektauswahl
- Overview
- Monitors
- Status Pages
- Incidents
- Maintenances
- Notification Channels
- Probes
- Automation
- Audit Log
- Settings

In 0.1 werden Organisationsfunktionen hinter dem automatisch erzeugten Default-Kontext verborgen. Navigationspunkte ohne Berechtigung werden ausgeblendet, der Server prüft dennoch jeden Request.

## 3. Zentrale Ansichten

### 3.1 Ersteinrichtung

Ein Wizard führt durch:

1. Sprache und Zeitzone
2. lokales Administratorkonto
3. optionalen Produktivhinweis bei SQLite
4. ersten Monitor
5. optionalen Benachrichtigungskanal

Die Einrichtung MUSS übersprungen und später fortgesetzt werden können. Zugangsdaten werden nie in URL, Analytics oder Browserpersistenz gespeichert.

### 3.2 Overview

Zeigt:

- Anzahl `DOWN`, `DEGRADED`, `UNKNOWN`, `MAINTENANCE`, `UP`
- aktive Alerts und Incidents
- letzte Zustandsänderungen
- Systemhinweise wie offline Probes, Scheduler-Lag, fehlgeschlagene Zustellungen oder anstehende Migration
- kompakte Uptime-Zusammenfassung

Eine grüne Gesamtdarstellung ist nur erlaubt, wenn keine unbekannten oder fehlerhaften Systemzustände verborgen sind.

### 3.3 Monitorliste

- Suche nach Name und Slug
- Filter nach Zustand, Typ, Tag, Probe, Verwaltungseigentum und Pause
- stabile Sortierung, serverseitige Pagination
- Spalten: Name, Zustand, Typ, letzter Check, Dauer, nächste Ausführung, Probe/Policy
- Massenaktionen: pausieren, fortsetzen, Kanal zuweisen, Tags ändern; Löschen nur mit expliziter Auswahlbestätigung
- Live-Änderungen aktualisieren Zeilen ohne Scrollposition oder Filter zu verlieren

Zustand wird nie ausschließlich über Farbe kommuniziert. Icon, Text und zugänglicher Name sind Pflicht.

### 3.4 Monitor erstellen/bearbeiten

Formularabschnitte:

1. Name, Slug, Beschreibung und Tags
2. Check-Typ und typspezifisches Ziel
3. Assertions
4. Intervall, Timeout, Retry und Recovery
5. Probe-Auswahl und Quorum ab 0.2
6. Benachrichtigungen
7. Vorschau und Test

„Test“ führt einen einmaligen Entwurf-Check aus, speichert den Monitor aber nicht. Das Ergebnis zeigt Phase, Dauer, redigierte Zielinformationen und klare Ursache. Ungespeicherte Secrets werden nur für diesen Check im Speicher gehalten.

Bei deklarativ verwalteten Monitoren ist das Formular standardmäßig read-only und verweist auf Quelle und letzten Apply. „Von Konfiguration lösen“ erfordert eine Warnung und passende Berechtigung.

### 3.5 Monitordetail

- aktueller Zustand, seit wann und warum
- letzte Observation mit Timing-Phasen
- Timeline von Zustandswechseln
- Uptime mit wählbaren 24 h, 7 d, 30 d, 90 d und benutzerdefiniertem Zeitraum
- getrennte Anteile für verfügbar, beeinträchtigt und ausgeschlossen
- Probe-Matrix ab 0.2
- aktive Silence/Maintenance
- Revisionen und Audit-Ereignisse
- Aktionen: Check starten, pausieren, quittieren, Silence, bearbeiten

### 3.6 Probes

Liste und Detail zeigen Verbindung, Version, Labels, Fähigkeiten, letzte Sichtung, Queue-Größe, Last und Zertifikatsablauf. Enrollment ist ein geführter Prozess mit Einmal-Code und fertigem Container-/Binary-Befehl. Der Code kann danach nicht erneut angezeigt werden.

### 3.7 Statusseiten

Editor mit:

- Titel, Slug, Logo/Farben, Locale
- Sichtbarkeit öffentlich oder Zugriffstoken
- Komponenten und Gruppen per zugänglicher Tastaturbedienung sortierbar
- auswählbare Historienzeiträume
- Vorschau ohne Veröffentlichung
- Incident- und Maintenance-Zuordnung

Öffentliche Seite ist serverseitig oder statisch vorgerendert, benötigt für den Erstinhalt kein JavaScript und setzt sinnvolle Cache Header. Sie enthält niemals Verwaltungsnavigation.

### 3.8 Incidents

Erstellen erfordert Titel, Auswirkung, betroffene Komponenten und erstes Update. Updates haben Status, Text und Zeit. Nach Veröffentlichung sind Updates unveränderlich; Korrekturen erfolgen durch ein neues Update. Resolve erfordert Abschlussnachricht.

### 3.9 Automation

Zeigt API-Tokens, CLI-Setup, OpenAPI-Link, letzte deklarative Applies und Drift. Der Plan-Viewer zeigt Create/Update/Delete gruppiert, Secret-Diffs nur als geändert und Löschungen deutlich separat.

## 4. Zustandsdarstellung

| Zustand | Semantik | UI-Hinweis |
|---|---|---|
| UP | Ziel nach Regeln verfügbar | „Verfügbar“ |
| DEGRADED | teilweise verfügbar oder Warnschwelle | „Beeinträchtigt“ |
| DOWN | echte Zielausfälle erfüllen Schwelle | „Nicht verfügbar“ |
| UNKNOWN | keine belastbare Aussage | „Unbekannt“ plus Ursache |
| PENDING | noch nicht ausreichend geprüft | „Wird geprüft“ |
| PAUSED | Nutzer hat Ausführung gestoppt | „Pausiert“ |
| MAINTENANCE | geplante Wartung aktiv | „Wartung“ |

Die UI DARF `UNKNOWN` nie grün darstellen oder in „alles in Ordnung“ einrechnen.

## 5. Interaktionsregeln

- Schreibende Aktionen liefern sofort sichtbares Pending-Feedback und werden nach Serverantwort bestätigt.
- Optimistische Updates sind nur für leicht rückgängig zu machende, konfliktarme Aktionen zulässig.
- Destruktive Aktionen nennen Ressource und Auswirkungen; Eingabe des Namens ist nur bei Organisation/Projekt oder vielen Ressourcen nötig.
- Fehler bleiben nahe am betroffenen Feld, zusätzlich existiert eine zugängliche Zusammenfassung.
- Technische Fehlercodes können kopiert werden; Standardtext bleibt verständlich.
- Formulare warnen vor Navigation mit ungespeicherten Änderungen.
- Alle Tabellenzustände sind in der URL abbildbar, damit Ansichten teilbar und nach Reload stabil sind.

## 6. Responsivität

- Verwaltungsoberfläche unterstützt 1280 px und größer vollständig sowie Tablets ab 768 px.
- Auf schmalen Displays sind Lesen, Quittieren, Silence und Incident-Update Pflicht; komplexe Builder dürfen vereinfacht sein.
- Öffentliche Statusseiten müssen ab 320 px ohne horizontales Scrollen funktionieren.

## 7. Barrierefreiheit

Muss-Kriterien:

- vollständige Tastaturbedienung
- sichtbarer Fokus
- semantische Überschriften und Landmarken
- Formularlabels und programmgesteuerte Fehlermeldungen
- Dialog-Fokusfalle und Rückgabe des Fokus
- Kontrast nach WCAG 2.2 AA
- `prefers-reduced-motion`
- Live-Updates werden zurückhaltend über ARIA angekündigt
- Charts besitzen textliche Zusammenfassung und tabellarisch abrufbare Werte
- Drag-and-drop besitzt Tastaturalternative

Axe-Prüfungen und manuelle Tastaturtests sind Release-Gates.

## 8. Internationalisierung

- Deutsch und Englisch sind vollständig ab 0.1 enthalten.
- Keine nutzerlesbaren Strings direkt in Komponenten.
- ICU Message Syntax für Plural und Variablen.
- Datum/Zeit über Locale und ausgewählte IANA-Zeitzone.
- APIs liefern stabile Codes und rohe Daten; UI lokalisiert.
- Layout muss mindestens 30 % längere Texte tolerieren.

## 9. Performancebudgets

Gemessen für Produktionsbuild bei 1000 Monitoren:

- Initiale komprimierte JS-Nutzlast der Login-/Overview-Route unter 250 KiB; weitere Bereiche lazy-loaded
- Largest Contentful Paint der öffentlichen Statusseite unter 2,5 s bei simuliertem Fast 3G und warmem Servercache
- Interaktion nach Routenwechsel unter 200 ms bei normaler API-Antwort
- Listen rendern nie alle Ressourcen ohne Virtualisierung/Pagination
- Keine Polling-Schleife unter 15 Sekunden; Live-Updates über SSE und kontrolliertes Refetch

## 10. Visuelle Leitlinie

Takt soll ruhig, präzise und technisch vertrauenswürdig wirken. Die Gestaltung verwendet eine neutrale Basis, klare Statusakzente und wenig dekorative Bewegung. Dark Mode und Light Mode sind Pflicht. Statusfarben sind als Design Tokens definiert und nicht frei pro Komponente gewählt.
