# Changelog

Verlauf dieses Forks. Neueste Einträge oben. Ist die Übergabe an den nächsten Agent.

## Offen

Die vollständige Liste mit Begründungen steht in `TODO.md`. Kurz:
Vorschau-Fenster beim Text-Kürzel, Oberfläche aufräumen (Louis findet sie
unübersichtlich, welcher Bildschirm genau ist noch zu klären), eigener
Update-Weg.


- **Deep-Link-Schema** heißt weiter `cap-desktop`. Kann jetzt umbenannt werden,
  die Anmeldung über cap.so hängt nicht mehr dran.
- **Update-Weg fehlt.** Der Updater ist ein Platzhalter, neue Versionen werden
  aus dem Quellcode gebaut.
- **Sidecar-Binaries** heißen intern noch `cap-cli`, `cap-exporter`, `cap-muxer`.
  Nicht sichtbar im normalen Betrieb, aber im App-Paket zu finden.
- **Spracherkennungs-Modelle** werden von `github.com/CapSoftware/transcription-models`
  geladen. Funktioniert, hängt aber an Caps Repo.
- Die Dateiendung von Projekten ist weiter `.cap`.
- `.env.example` fehlt.
- Der Absturz-Fix umgeht einen Fehler in Tauri, statt ihn zu beheben. Wenn Tauri
  angehoben wird, prüfen, ob `vendor/tauri-runtime-wry` wieder wegfallen kann.

## 2026-08-18 (Trennung von Cap)

Shelf ist ab jetzt eigenständig und lokal. Nichts geht mehr an Cap.

- **Kommandozeile:** Konto-, Agent-, Zugangsdaten-, Analytics- und
  Organisations-Befehle sind gelöscht, ebenso der Upload-Auslöser. Übrig sind
  Aufnehmen, Screenshots, Export, Ziele und lokale Automatisierungen. Hilfetexte
  und Beispiele sagen `shelf`.
- **Logo:** Die Wortmarke im Hauptfenster ist das Shelf-Zeichen mit Schriftzug,
  hell und dunkel.

- **Nichts verlässt mehr den Rechner:** Sentry-Absturzmeldungen, OpenPanel-
  Nutzungsdaten und OpenTelemetry-Traces sind entfernt, samt Abhängigkeiten und
  `t.cap.so` in der Fenster-Sicherheitsregel.
- **Updater** zeigte auf Caps CDN und hätte Shelf beim nächsten Release durch Cap
  ersetzt. Jetzt ein Platzhalter, der nichts abruft.
- **Konten und Cloud raus:** `auth.rs`, `web_api.rs`, `upload.rs`, `api.rs` und
  `logging.rs` gelöscht, neun Befehle entfernt (Hochladen, Teilen-Links,
  Log-Versand, Plan-Abfragen, Server-URL). Cap-Pro-Fenster und OAuth-Plugin weg.
- **Sofort-Aufnahmen bleiben lokal:** Sie nehmen weiter auf und speichern, nur der
  Upload ist weg. Die Auflösungs-Sperre, die Cap Pro verlangte, ist ersatzlos weg.
- **Oberfläche:** Anmelde-Knopf, Teilen-Knopf, Lizenz-, Feedback- und
  Integrations-Seiten (Google Drive, S3) entfernt.
- **Repo verkleinert:** Caps Website, Mobile-App, Chrome-Erweiterung, Discord-Bot,
  Media-Server, Storybook, Web-Cluster und Cloud-Infrastruktur gelöscht, dazu elf
  Release-Abläufe. Übrig ist eine schlanke CI für die Desktop-App.
- **Namen:** Aufnahmen heißen `Shelf JJJJ-MM-TT at HH.MM.SS` (alte Cap-Namen
  werden weiter erkannt), Terminal-Befehl `shelf` in `~/.shelf/bin`, Menüleiste,
  Absturzhinweis und Import-Meldungen tragen den neuen Namen. Der Browser-Rahmen
  im Editor brennt kein `cap.so` mehr in exportierte Videos.
- **Bleibt bewusst:** `LICENSE` (AGPLv3) und `README.cap.md`. Die Lizenz verlangt,
  dass die Herkunft erkennbar bleibt. Das ist Rechtstext, keine Cap-Werbung.

## 2026-08-18 (Umbenennung)

- Aus dem Fork wird **Shelf**. Produktname, Bundle-ID (`de.shelf.desktop`, Dev:
  `de.shelf.desktop.dev`), Fenstertitel, die Standard-Ausschlussliste beim
  Aufnehmen und der Log-Ordner tragen den neuen Namen.
- Neues App-Icon: ein Regal mit zwei Karten darauf, reine Geometrie in Schiefer
  und Blau, transparent, bei 32 Pixeln noch lesbar. Erzeugt aus
  `scripts` heraus als exakte Form, nicht als KI-Bild (der KI-Versuch kam mit
  grünem Hintergrund zurück).
- **Achtung:** Die neue Bundle-ID bedeutet, dass macOS die Rechte für
  Bildschirmaufnahme und Bedienungshilfen neu abfragt und dass alte Aufnahmen
  und Einstellungen unter `so.cap.desktop` liegen bleiben.

## 2026-08-18

- **Review-Funde (Codex) behoben:** `start_file_drag` nahm jeden lesbaren Pfad auf
  dem Rechner an, jetzt nur noch Dateien aus Caps eigenen Ordnern (Aufnahmen und
  Screenshots). Statt `std::sync::mpsc::recv()` in einer async-Funktion wartet der
  Befehl jetzt auf einen `tokio::sync::oneshot`. Der Lese-Notausgang im
  Fensterspeicher zählt jetzt echte Schreiber mit: liegt ein echter Schreibzugriff
  an, wird die Nachricht übersprungen, direkt gelesen wird nur, wenn die Sperre
  ohne Schreiber hängt. Der mitkopierte `Cargo.lock` im Vendor-Ordner ist raus.

- **Absturz behoben:** Sobald sich das Aufnahme-Overlay öffnete, starb die App mit
  `already mutably borrowed: BorrowError` in `tauri-runtime-wry`. Ursache liegt in
  Tauri: der Fensterspeicher ist ein `RefCell` hinter einem `unsafe impl Sync`,
  und seine Borrow-Sperre kann hängen bleiben, obwohl niemand schreibend zugreift
  (tauri-apps/tauri#14801, tauri-apps/tauri#15003). Danach bricht jeder weitere
  Fensterzugriff auf dem Haupt-Thread ab.
  Gemessen mit einer instrumentierten Kopie von `tauri-runtime-wry`: kein einziger
  schreibender Zugriff im Crate, und der Fensterspeicher selbst ist inhaltlich
  intakt (`map_len=2`, korrekte Labels). Nur die Sperre steht falsch.
  Lösung: `tauri-runtime-wry 2.8.1` liegt jetzt unter `vendor/` (gleiches Muster
  wie `vendor/tao` und `vendor/wgpu-hal`), alle lesenden Zugriffe laufen über
  `win_borrow()`. Hängt die Sperre, wird einmal geloggt und die Karte direkt
  gelesen, statt die App zu killen.
  Geprüft: drei Screenshots hintereinander, kein Absturz, Overlay sichtbar,
  Hit-Test-Listener läuft weiter.

- **Screenshots im Overlay + Drag-out** (`feat/screenshot-overlay-drag-out`):
  Neue Einstellung „Nach einem Screenshot" (Editor öffnen / Standard / im Overlay
  zeigen). Screenshot-Karten im Overlay lassen sich per Maus in andere Apps
  ziehen (`start_file_drag`, Crate `drag`).

- **Overlay-Karten auf Monitoren mit anderer Skalierung** (`fix/overlay-hit-test-mixed-dpi`):
  `Window::cursor_position` ist auf macOS mit dem Faktor des Hauptmonitors
  skaliert, `outer_position` dagegen mit dem des eigenen Monitors. Dadurch wurden
  Karten auf einem zweiten Monitor nie anklickbar. Hit-Test rechnet jetzt in
  logischen Punkten. Zusätzlich: Karten, die entstehen, während das Overlay
  versteckt ist, messen sich als 0x0 und meldeten nie eine klickbare Fläche —
  sie werden jetzt neu vermessen, sobald das Fenster sichtbar wird.

## Vorgeschichte

Fork von [CapSoftware/Cap](https://github.com/CapSoftware/Cap), geklont am
2026-08-17. Lizenz bleibt AGPLv3 (siehe `LICENSE`), Teile MIT.
