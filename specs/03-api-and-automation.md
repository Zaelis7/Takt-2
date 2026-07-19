# 03 – API und Automatisierung

## 1. API-Grundsätze

- Basis-URL: `/api/v1`
- Format: JSON mit UTF-8
- Vertrag: OpenAPI 3.1 in `contracts/openapi.yaml`
- Zeitpunkte: RFC 3339 UTC, etwa `2026-07-19T12:30:00Z`
- IDs: UUIDv7 als Strings
- Feldnamen: `snake_case`
- Unbekannte Eingabefelder werden mit `400` abgelehnt. Antwort-Clients müssen zusätzliche Felder tolerieren.
- Alle Antworten enthalten `X-Request-Id`; ein eingehender gültiger Wert darf übernommen werden.
- Breaking Changes erfordern `/api/v2`; neue optionale Felder und neue Endpunkte sind innerhalb v1 zulässig.

Der Vertrag in `openapi.yaml` enthält die Kernressourcen. Jeder neue produktive Endpunkt MUSS zuerst oder in demselben Change im Vertrag spezifiziert werden.

## 2. Authentifizierung

### Browser

- Kurzlebige serverseitige Session mit zufälliger ID im Cookie
- Cookie: `HttpOnly`, `Secure` außerhalb explizitem Localhost-Modus, `SameSite=Lax`, Pfad `/`
- CSRF-Token für schreibende Requests
- Session-Rotation nach Login, Rechteänderung und sensiblen Aktionen

### API

- Persönliche oder Service-Account-Tokens als `Authorization: Bearer <token>`
- Tokenwert mindestens 256 Bit Entropie, nur einmal bei Erzeugung sichtbar
- Speicherung nur als langsamer Hash mit separatem Token-Präfix zur Suche
- Scopes folgen `resource:verb`, z. B. `monitors:read`, `monitors:write`, `checks:execute`
- Token kann auf Organisation, Projekt, Ablaufdatum und IP-Netzbereiche eingeschränkt werden

## 3. Autorisierung

Jeder Application Use Case erhält `Actor`, `OrganizationId`, optional `ProjectId` und erforderliche Permission. Repository-Filter allein sind kein ausreichender Schutz.

| Rolle | Kernrechte |
|---|---|
| owner | Organisation, Abrechnungsvorbereitung, alle Projekte, Owner verwalten |
| admin | Projekte, Mitglieder, Probes, Integrationen verwalten |
| editor | Monitore, Kanäle, Statusseiten, Incidents ändern |
| operator | Checks auslösen, Incidents aktualisieren, Alerts quittieren, Silences setzen |
| viewer | lesen und exportieren, keine Secrets |

## 4. Ressourcenmuster

### Erstellen

`POST /api/v1/projects/{project_id}/monitors`

- Client SOLL `Idempotency-Key` senden.
- Antwort `201 Created`, `Location` und vollständige Ressource.
- Wiederholung mit demselben Key und identischem Body liefert dieselbe semantische Antwort.
- Wiederholung mit abweichendem Body liefert `409 idempotency_key_reused`.

### Lesen und Ändern

- `GET` liefert `ETag: "<version>"`.
- `PATCH` verwendet JSON Merge Patch und SOLL `If-Match` senden.
- Falsches ETag liefert `412 version_conflict` und den aktuellen ETag.
- `PUT` wird nur für vollständige, natürlich ersetzbare Subressourcen verwendet.

### Löschen

- `DELETE` liefert `204`.
- Bei referenzierten Ressourcen liefert die API `409 resource_in_use` mit maschinenlesbaren Referenzen.
- `force=true` ist für fachliche Ressourcen verboten; abhängige Änderungen müssen explizit erfolgen.

### Listen

```json
{
  "items": [],
  "next_cursor": "opaque-or-null"
}
```

- Parameter: `limit` 1–200, `cursor`, `sort`, ressourcenspezifische Filter.
- Cursor sind opak, signiert und an Filter/Sortierung gebunden.
- Standardreihenfolge ist `created_at asc, id asc`, sofern der Endpunkt nichts anderes spezifiziert.
- Ungültiger oder abgelaufener Cursor liefert `400 invalid_cursor`.

## 5. Problem Details

Fehler verwenden `application/problem+json`:

```json
{
  "type": "https://takt.dev/problems/validation_failed",
  "title": "Validation failed",
  "status": 422,
  "code": "validation_failed",
  "detail": "One or more fields are invalid.",
  "instance": "/api/v1/projects/.../monitors",
  "request_id": "019b...",
  "errors": [
    { "path": "/spec/url", "code": "invalid_url", "message": "A valid HTTP URL is required." }
  ]
}
```

`title` und `detail` dürfen lokalisiert werden; `code` und Feldcodes bleiben stabil.

## 6. Kernendpunkte bis 0.3

### System und Identität

- `GET /health/live`, `GET /health/ready`
- `GET /api/v1/system/info`
- `/api/v1/auth/login`, `/logout`, `/session`
- CRUD `/api/v1/api-tokens`
- CRUD `/api/v1/organizations`, `/projects`, `/memberships`

### Monitoring

- CRUD `/api/v1/projects/{project_id}/monitors`
- `POST /monitors/{id}/checks`
- `GET /monitors/{id}/observations`
- `GET /monitors/{id}/transitions`
- `GET /monitors/{id}/uptime?from=&to=&bucket=`
- `POST /monitors/{id}/pause`, `POST /monitors/{id}/resume`
- CRUD `/api/v1/probes` und Enrollment-Aktionen
- CRUD `/maintenances`, `/silences`, `/notification-channels`
- `POST /notification-channels/{id}/test`

### Statuskommunikation

- CRUD `/status-pages`, `/incidents`
- `POST /incidents/{id}/updates`
- Öffentliche Projektion unter `/api/public/v1/status-pages/{slug}`
- Abonnieren, Bestätigen und Abmelden über nicht erratbare Einmal-Tokens

### Automatisierung

- `POST /api/v1/config/validate`
- `POST /api/v1/config/plan`
- `POST /api/v1/config/apply`
- `GET /api/v1/audit-events`
- `GET /api/v1/events/stream` für authentifizierte SSE

## 7. Asynchrone Aktionen

Schnelle Änderungen antworten synchron. Checks, Import, größere Applies und Exporte verwenden Operations:

```json
{
  "id": "019b...",
  "kind": "monitor.check",
  "status": "queued",
  "created_at": "2026-07-19T12:30:00Z",
  "links": { "self": "/api/v1/operations/019b..." }
}
```

Status: `queued`, `running`, `succeeded`, `failed`, `cancelled`. Operations besitzen Fortschritt, Ergebnislinks und Problem Details, aber keine Secrets. Abgeschlossene Operations werden mindestens 24 Stunden gehalten.

## 8. Rate Limits

- Authentifizierte Lesezugriffe: standardmäßig 600 Requests/Minute je Actor
- Schreibzugriffe: 120/Minute je Actor
- Login: 10/Minute je IP und Konto mit zunehmender Verzögerung
- Push Heartbeats: passend zum Monitorintervall plus Burst, nie global unbegrenzt
- Öffentliche Statusseiten: 120/Minute je IP mit Cache-Unterstützung
- `429` enthält `Retry-After` und `rate_limit_exceeded`

Grenzen sind konfigurierbar, aber nicht deaktivierbar für Login und Enrollment.

## 9. `taktctl`

CLI-Befehle:

```text
taktctl login
taktctl context use <name>
taktctl validate -f takt.yaml
taktctl plan -f takt.yaml [--prune]
taktctl apply -f takt.yaml [--prune] [--yes]
taktctl export --project <slug> -o takt.yaml
taktctl monitor run <slug> --wait
taktctl import uptime-kuma analyze|plan|apply
```

- Kontexte liegen im OS-spezifischen Konfigurationsverzeichnis mit restriktiven Dateirechten.
- Token können alternativ aus stdin, Umgebungsvariable oder Secret Store gelesen werden.
- `--output json` liefert ausschließlich maschinenlesbare Daten auf stdout; Logs gehen nach stderr.
- Exit Codes: `0` Erfolg/kein Diff, `2` valider Plan mit Änderungen, `3` Validierungsfehler, `4` Auth/Permission, `5` Konflikt, `10` Infrastrukturfehler.
- Nicht-interaktive Befehle fragen nie verdeckt nach Eingaben.

## 10. Deklarative Konfiguration

Das Dokument folgt `contracts/takt-config.schema.json` und verwendet:

```yaml
apiVersion: takt.dev/v1alpha1
kind: TaktProject
metadata:
  organization: acme
  project: production
spec: {}
```

### Verwaltungseigentum

- Jede angewendete Ressource erhält `managed_by=declarative` und einen stabilen Hash der Quellidentität.
- Felder werden zunächst vollständig von einer Quelle verwaltet; Feldmanager-Merging gehört nicht zu 0.3.
- Eine zweite Quelle darf dieselbe Ressource nicht übernehmen, bevor dies explizit bestätigt wurde.
- Änderungen über UI/API an deklarativ verwalteten Feldern werden entweder verweigert oder als kontrollierter „Detach“ angeboten; nie still überschrieben.

### Plan

Plan-Ausgabe besitzt `create`, `update`, `delete`, `unchanged`, Warnungen und normalisierte Feld-Diffs. Geheime Werte werden nur als `changed: true` gezeigt.

### Apply

- Validiert vollständig vor der ersten Änderung.
- Verwendet eine serverseitige Operation und eine Config-Revision.
- Ist pro Projekt atomar, soweit externe Aktionen nicht beteiligt sind.
- `--prune` benötigt dieselbe Quellidentität und zeigt Löschungen bereits im Plan.
- Apply eines unveränderten Dokuments erzeugt keine Revision und keine Benachrichtigung.

## 11. Webhooks und Ereignisse

Ausgehende generische Webhooks verwenden CloudEvents 1.0 als strukturiertes JSON. Mindesttypen:

- `takt.monitor.state_changed.v1`
- `takt.alert.opened.v1`, `.acknowledged.v1`, `.resolved.v1`
- `takt.incident.created.v1`, `.updated.v1`
- `takt.maintenance.started.v1`, `.ended.v1`

Requests enthalten Event-ID, Zeitstempel und HMAC-SHA256-Signatur über den unveränderten Body. Wiederholungen behalten dieselbe Event-ID.

## 12. API-Kompatibilität

- Entfernen oder Umbenennen eines Feldes, Verschärfen einer Validierung oder Ändern einer Enum-Semantik ist breaking.
- Neue Enum-Werte sind nur zulässig, wenn der Vertrag Clients ausdrücklich zu tolerantem Lesen verpflichtet; andernfalls neue Version.
- Eine Deprecation wird mindestens zwei Minor-Releases dokumentiert und per `Deprecation`/`Sunset` Header signalisiert.
- CI vergleicht OpenAPI gegen das letzte Release und blockiert nicht erlaubte Breaking Changes.
