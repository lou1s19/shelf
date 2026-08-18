# Changelog

Verlauf dieses Forks. Neueste Einträge oben. Ist die Übergabe an den nächsten Agent.

## Offen

- **Menüleisten-Icons** sind noch die von Cap (`icons/tray-*.png`, fünf Zustände).
- **Update-Weg fehlt.** Caps Updater-Endpunkt ist abgeschaltet, weil er Shelf
  sonst durch Cap-Builds ersetzt hätte. Eigener Release-Feed steht aus.
- **Texte in der Oberfläche** sagen an vielen Stellen noch „Cap" (rund 120
  Stellen im Frontend). Fenstertitel und App-Identität sind schon umgestellt.
- Das Deep-Link-Schema heißt weiter `cap-desktop`. Umbenennen würde die Anmeldung
  über cap.so zerlegen.
- `.env.example` fehlt. Für den Desktop-Build reicht bisher eine kleine `.env` im
  Repo-Root, deren Variablen sind noch nicht dokumentiert.
- Der Absturz-Fix umgeht einen Fehler in Tauri, statt ihn zu beheben. Wenn Tauri
  angehoben wird, prüfen, ob `vendor/tauri-runtime-wry` wieder wegfallen kann.

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
