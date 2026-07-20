# 04 – Probes, Scheduler und Check-Spezifikationen

## 1. Ziel

Der Checkpfad soll viele unabhängige Netzwerkprüfungen zuverlässig ausführen, ohne Zielausfälle mit eigenen Infrastrukturfehlern zu vermischen. Lokale und entfernte Ausführung verwenden dieselbe fachliche `CheckSpec` und dasselbe Observation-Modell.

## 2. Scheduler

### Planung

- Nächste Ausführung basiert auf dem geplanten Zeitpunkt, nicht auf dem Abschluss des letzten Checks; so entsteht kein kumulativer Drift.
- Deterministischer Jitter verteilt Last. Der Jitter-Schlüssel basiert auf Monitor-ID und Zeitfenster.
- Es ist standardmäßig höchstens ein Job pro Monitor und Probe aktiv.
- `overlap_policy` ist bis 0.3 fest `skip`; ein überfälliger vorheriger Check erzeugt keinen parallelen Check.
- Nach längerer Serverpause werden verpasste Intervalle nicht vollständig nachgeholt. Es wird höchstens ein sofortiger Check geplant.

### Leases

- Jeder Job hat `not_before`, `deadline` und `lease_until`.
- Eine Probe bestätigt Empfang. Nicht bestätigte Jobs können nach Lease-Ablauf neu zugeordnet werden.
- Doppelte Observationen werden gespeichert oder verworfen, aber nur eine wird als kanonisch ausgewertet.
- Änderungen eines Monitors erzeugen eine neue Revision. Ausstehende Jobs alter Revisionen werden `superseded`.

### Überlast

- Globale und per-Probe Concurrency Limits verhindern Ressourcenerschöpfung.
- Fairness erfolgt mindestens pro Projekt und Probe.
- Scheduler-Lag wird gemessen. Übersteigt der p99-Lag 30 Sekunden, wird ein sichtbarer Systemzustand ausgelöst.
- Load Shedding verwirft keine Ergebnisse und erzeugt keinen `DOWN`; Jobs werden verspätet oder `UNKNOWN`.

## 3. Probe-Lebenszyklus

### Enrollment

1. Administrator erstellt einen Einmal-Enrollment-Code mit Organisation, Labels und Ablaufzeit maximal 15 Minuten.
2. Probe generiert lokal ein Schlüsselpaar und sendet CSR plus Code.
3. Server stellt ein kurzlebiges Client-Zertifikat aus und bindet es an die Probe-ID.
4. Probe speichert privaten Schlüssel mit restriktiven Rechten.
5. Probe verbindet sich ausschließlich ausgehend zum Server.

Zertifikate laufen standardmäßig nach 30 Tagen ab und werden ab sieben Tagen vorher automatisch rotiert. Sperrung wirkt spätestens bei der nächsten Verbindung.

### Verbindung

- gRPC über HTTP/2 und TLS 1.3; mTLS ist verpflichtend außerhalb explizitem lokalen Testmodus.
- Probe sendet Hello mit Version, Fähigkeiten, Labels und aktueller Last.
- Server akzeptiert nur kompatible Protokollversionen und liefert klare Upgrade-Hinweise.
- Heartbeat alle 15 Sekunden; nach 45 Sekunden ohne Heartbeat `disconnected`, nach konfigurierbarer Stale-Zeit `offline`.
- Steuerverbindung ist bidirektional, aber vom Probe-Prozess initiiert.

### Offline-Puffer

- Probe puffert angenommene Observationen verschlüsselt auf lokaler Disk.
- Standardlimit: 100.000 Ergebnisse oder 1 GiB, je nachdem was zuerst erreicht wird.
- Volle Queue verwirft zuerst die ältesten bereits bestätigten Daten, niemals unbestätigte Daten. Ist kein Platz verfügbar, nimmt die Probe keine neuen Jobs an.
- Wiederanlieferung ist idempotent und in zeitlich geordneten Batches.
- Probe führt keine Checks nach deren Deadline aus, nur weil sie offline war.

## 4. Gemeinsame Check-Regeln

- `timeout_ms` umfasst den gesamten Check und darf das Intervall nicht überschreiten.
- Einzelphasen erhalten eigene kleinere Budgets und werden gemessen.
- Abbruch wird kooperativ propagiert.
- Fehler werden in stabile Codes und redigierte, nutzerlesbare Zusammenfassungen übersetzt.
- IP-Adresse des tatsächlichen Verbindungsziels darf intern gespeichert werden, aber nicht automatisch öffentlich erscheinen.
- Proxy, DNS-Resolver und Netzwerkfamilie sind explizite Optionen.
- Redirects, TLS-Prüfung und Body-Limits haben sichere Defaults.
- Geheimnisse werden kurz vor Ausführung aufgelöst und nicht in Jobs, Logs oder Observations serialisiert.

## 5. Check-Typen für 0.1

### 5.0 Kanonisches CheckSpec-Mapping

Die Rust-Domäne und OpenAPI verwenden `snake_case`; das deklarative Schema behält seine vorhandene `camelCase`-Konvention. Proto verwendet dieselben `snake_case`-Feldnamen wie die Domäne. Die folgende Abbildung ist normativ; ein Adapter darf keine weitere Option oder abweichende Einheit einführen:

| Typ | Domäne/OpenAPI/Proto | Deklarative Config | Einheit, Default und Grenze |
|---|---|---|---|
| HTTP | `url`, `method` | `url`, `method` | URL maximal 2048 Zeichen; Methode `GET` |
| HTTP | `headers`, `body` | `headers`, `body` | höchstens 50 Header; Literalwerte maximal 8192 Zeichen, Body maximal 1 MiB; alternativ Secret-Referenz |
| HTTP | `expected_status_min`, `expected_status_max` | `expectedStatus.min`, `.max` | `200`/`399`, jeweils 100–599 und Minimum nicht größer als Maximum |
| HTTP | `follow_redirects`, `verify_tls`, `http_version` | `followRedirects`, `verifyTls`, `httpVersion` | `5` (0–10), `true`, `auto`; Version `auto`, `http1_1` oder `http2` |
| HTTP | `body_contains`, `body_matches` | `bodyContains`, `bodyMatches` | optional; Assertionseingabe jeweils maximal 4 MiB |
| HTTP | `json_pointer_equals`, `json_pointer_contains` | `jsonPointerEquals`, `jsonPointerContains` | optionales Paar aus Pointer (1–1024 Zeichen) und Vergleichswert (maximal 4 MiB) |
| HTTP | `max_response_time_ms` | `maxResponseTime` | optional; Millisekunden beziehungsweise Config-Dauer, 1–300000 ms |
| HTTP | `response_body_limit_bytes` | `responseBodyLimitBytes` | Bytes, Standard und Maximum 1048576 |
| HTTP | `auth` | `auth` | `basic`, `bearer` oder `mtls`; Zugangsdaten ausschließlich als Secret-Referenzen |
| HTTP Auth | `username`, `password`, `token`, `client_certificate`, `client_key` | `username`, `password`, `token`, `clientCertificate`, `clientKey` | genau die zur gewählten Auth-Art gehörenden SecretRefs; keine Literalwerte |
| TCP | `host`, `port`, `send_bytes`, `expect_prefix` | `host`, `port`, `sendText`, `expectPrefix` | Port 1–65535; Config-Text wird UTF-8-kodiert, je maximal 4096 Bytes |
| DNS | `name`, `record_type`, `expected_values`, `resolver` | `name`, `recordType`, `expectedValues`, `resolver` | Typ A/AAAA/CNAME/MX/TXT/NS/SOA/CAA; maximal 100 Werte zu je 4096 Zeichen; Resolver-URI mit `udp`/`tcp`/`tls` |
| DNS | `expected_rcode`, `minimum_answers`, `value_match` | `expectedRcode`, `minimumAnswers`, `valueMatch` | Defaults `NOERROR`, `1`, `contains`; Antworten 0–100 |
| ICMP | `host`, `packets`, `required_successes`, `max_latency_ms` | `host`, `packets`, `requiredSuccesses`, `maxLatencyMs` | Pakete `3` (1–5), Erfolge `1` (1–Pakete), optionale Latenz 1–300000 ms |
| TLS | `host`, `port`, `server_name`, `warning_days`, `critical_days` | `host`, `port`, `serverName`, `warningDays`, `criticalDays` | Port `443`; Schwellen `30`/`7` Tage, 0–3650 und kritisch nicht größer als Warnung |
| Push | `grace_ms`, `allow_get` | `grace`, `allowGet` | 60000 ms beziehungsweise `60s`, 0–86400000 ms; `false` |
| Browser | `start_url`, `steps` | `startUrl`, `steps` | 1–20 deklarative Schritte; Action-Namen sind in Config camelCase und sonst snake_case |
| Browser Step | `action`, `selector`, `value` | `action`, `selector`, `value` | `navigate`, `click`, `fill`, `wait`, Text-/URL-/Status-Assertion; `fill` erfordert Selector und SecretRef |
| Browser | `max_network_response_bytes`, `screenshot_on_failure_max_bytes` | `maxNetworkResponseBytes`, `screenshotOnFailureMaxBytes` | Defaults 10 MiB/1 MiB, Maximum jeweils 10 MiB; Screenshotwert 0 deaktiviert das Artefakt |

Config-Secret-Referenzen bestehen aus `secretRef` und optionalem `key` (Standard `value`), OpenAPI aus `secret_ref` und `key`. Vor Probe-Dispatch MUSS der Server sie zuletzt verantwortlich auflösen und nur den kurzlebigen `ephemeral_key` im versiegelten Secret-Bundle und in Proto-Nachrichten verwenden. Literal-Secrets in `auth` oder Browser-`fill` sind ungültig. Proto-Felder mit einem von null verschiedenen Standard und zugleich sinnvoller expliziter Null-/False-Semantik verwenden `optional`; andere fehlende numerische Werte und leere optionale String-Defaults werden vor Eintritt in die Domäne auf die Tabelle normalisiert. Leere Pflichtstrings bleiben ungültig. Ungültige Proto-Werte ergeben `ACK_DISPOSITION_REJECTED_INVALID`, niemals eine Observation mit `TARGET_FAILURE`.

### 5.1 HTTP/HTTPS

Pflicht: `url`.

Optionen:

- Methode `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS`
- Header und optionaler Body; Secret-Werte nur als Secret-Referenzen
- erlaubte Statusbereiche, Standard `200..399`
- Redirects 0–10, Standard 5
- TLS-Verifikation standardmäßig an; explizite unsichere Deaktivierung erzeugt dauerhafte Warnung
- HTTP-Version automatisch, optional 1.1 oder 2 bevorzugt
- Body-Leselimit 1 MiB, Assertion-Limit 4 MiB; größere Werte werden abgelehnt
- Assertions: substring, regulärer Ausdruck, JSON Pointer gleich/enthält, Antwortzeit
- Auth: Basic, Bearer, mTLS über Secret-Referenzen

Erfasste Phasen: DNS, connect, TLS, time-to-first-byte, total. Header und Body werden nicht vollständig persistiert.

### 5.2 TCP

Pflicht: `host`, `port`. Verbindungsaufbau reicht standardmäßig als Erfolg. Optional können begrenzte Bytes gesendet und ein Antwortpräfix erwartet werden. Maximal je 4 KiB, keine Skriptsprache.

### 5.3 DNS

Pflicht: `name`, `record_type`. Unterstützt A, AAAA, CNAME, MX, TXT, NS, SOA, CAA. Optional eigener Resolver über UDP/TCP/DoT. Assertions auf RCODE, Mindestanzahl und exakte/enthaltene normalisierte Werte.

### 5.4 ICMP

Pflicht: `host`. Bis zu fünf Pakete; Erfolgsschwelle und maximale Latenz konfigurierbar. Fehlen notwendige Betriebssystemrechte, wird der Monitor mit `capability_missing` zu `UNKNOWN`, nicht `DOWN`. Container-Dokumentation beschreibt die minimale Capability.

### 5.5 TLS-Zertifikat

Pflicht: `host`, `port`, Standard 443. Prüft Handshake, Hostname, Kette und Ablauf. Warn- und Kritisch-Schwellen in Tagen, Standard 30/7. Ergebnis kann `DEGRADED` vor Ablauf und `DOWN` bei ungültigem/abgelaufenem Zertifikat sein.

### 5.6 Push-Heartbeat

- Jeder Monitor besitzt einen rotierbaren, mindestens 256 Bit starken Push-Token.
- Endpunkt akzeptiert `POST`; `GET` kann zur Kompatibilität optional aktiviert werden.
- Nutzlast maximal 16 KiB mit optionalem Status `up`, `down`, `degraded`, Dauer und nicht sensiblem Text.
- Erwartungsfenster plus Grace Period bestimmt den Zustand.
- Token erscheint nie in Listen, Logs oder Statusseiten.
- Rotation kann alten und neuen Token höchstens 24 Stunden überlappen lassen.

## 6. Browsercheck für 0.3

Browserchecks laufen in einem separaten `takt-browser-worker` oder einem dedizierten Probe-Pool:

- Chromium in einem nicht privilegierten Container/Sandbox
- read-only Root-Dateisystem, temporäres begrenztes Profil, kein Host-Mount
- Netzwerk-Egress nach definierter Policy
- Szenario ist deklarativ: navigate, click über stabile Locator, fill über Secret-Referenz, wait, assert text/url/status
- kein beliebiges JavaScript in 0.3
- maximal 20 Schritte, 60 Sekunden und 10 MiB Netzwerkantworten pro Check
- Screenshot nur bei Fehler, verschlüsselt und standardmäßig nach sieben Tagen gelöscht
- Browsercrash ist `PROBE_FAILURE`, nicht `TARGET_FAILURE`

## 7. Multi-Probe-Auswertung ab 0.2

Probe-Auswahl verwendet Labels, etwa `region=eu-central` und `network=public`. Ein Monitor speichert Selektor und Policy:

- `any`: mindestens ein gültiger Standort erfolgreich; wenn keiner erfolgreich und mindestens ein Target-Ausfall vorliegt, `DOWN`; fehlende eindeutige Evidenz `UNKNOWN`.
- `all`: jeder erreichbare ausgewählte Standort muss erfolgreich sein; Target-Ausfall eines Standorts ergibt `DOWN`, optional `DEGRADED` bei konfigurierter Toleranz.
- `quorum`: mindestens `min_success` erfolgreiche Standorte. Wird das Quorum durch echte Target-Ausfälle verfehlt, `DOWN`; kann es wegen fehlender Probes nicht entschieden werden, `UNKNOWN`.

Ergebnisse gehören zu einem Auswertungsfenster. Standard ist `max(2 * timeout, 15s)`, darf aber das Monitorintervall nicht überschreiten.

## 8. SSRF- und Egress-Regeln

Monitoring muss private Ziele erreichen können; pauschales Blockieren privater Netze ist daher falsch. Stattdessen:

- Egress Policy pro Probe/Projekt mit erlaubten oder verbotenen CIDRs, Ports und DNS-Suffixen
- Standard für lokale Probe: private und öffentliche Ziele erlaubt, Link-local, Metadata-Endpunkte und Server-Control-Plane verboten
- Standard für gemeinsam genutzte Probe: nur explizit erlaubte Ziele
- DNS-Auflösung wird vor Verbindung geprüft; jede Redirect-Auflösung erneut
- Schutz gegen DNS Rebinding, IPv4-in-IPv6-Darstellungen und alternative IP-Schreibweisen
- Keine Weitergabe sensibler Header über Domain- oder Scheme-Wechsel bei Redirect

## 9. Probe-Protokollkompatibilität

`contracts/probe.proto` definiert Major-Version 1. Neue optionale Felder bleiben kompatibel. Server unterstützt mindestens die Probe-Versionen der aktuellen und vorherigen Minor-Version. Bei inkompatibler Version nimmt die Probe keine Jobs an, bleibt aber im UI mit Upgradegrund sichtbar.

## 10. Ressourcen- und Leistungsziele

Auf Referenzhardware mit 2 vCPU und 1 GiB RAM, lokaler PostgreSQL-Verbindung und simulierten Zielen:

- 0.1: 1.000 aktive 60-Sekunden-Monitore; p99 Scheduler-Lag unter 5 s; Server-RSS unter 500 MiB.
- 0.2: 5.000 aktive 60-Sekunden-Monitore über fünf Probes; p99 Dispatch-Lag unter 5 s; keine verlorene angenommene Observation bei 15 Minuten Verbindungsabbruch.
- Eine einzelne Probe mit 1 vCPU/256 MiB soll 500 überwiegend wartende Netzwerkchecks bei 60-Sekunden-Intervall verarbeiten.
- Notification Backlog beeinflusst Scheduler-Lag nicht messbar um mehr als 10 %.

Die Benchmarks verwenden reproduzierbare simulierte Ziele und dokumentierte Hardware. Sie sind Kapazitätsziele, keine Marketingaussagen für beliebige Checks.
