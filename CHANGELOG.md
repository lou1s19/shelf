# Changelog

Verlauf dieses Forks. Neueste Einträge oben. Ist die Übergabe an den nächsten Agent.

## Offen

Die vollständige Liste mit Begründungen steht in `TODO.md`. Kurz:
Vorschau-Fenster beim Text-Kürzel, eigener Update-Weg.

- **Update-Weg fehlt.** Der Updater ist ein Platzhalter, neue Versionen werden
  aus dem Quellcode gebaut.
- **Rust-Crates heißen weiter `cap-*`**, die npm-Pakete `@cap/*`, die
  Sidecar-Binaries `cap-cli`, `cap-exporter`, `cap-muxer`. Rein intern, aber im
  App-Paket zu finden. Eine Umbenennung ist mechanisch, aber breit.
- **Spracherkennungs-Modelle** werden von `github.com/CapSoftware/transcription-models`
  geladen. Funktioniert, hängt aber an Caps Repo.
- Die Dateiendung von Projekten ist weiter `.cap`.
- Der Absturz-Fix umgeht einen Fehler in Tauri, statt ihn zu beheben. Wenn Tauri
  angehoben wird, prüfen, ob `vendor/tauri-runtime-wry` wieder wegfallen kann.
- Aus dem Fehler-Audit noch offen, bewusst nicht angefasst, weil größere
  Umbauten: der Schreib-Lock im Screenshot-Editor wird über die ganze
  Erzeugung gehalten (`screenshot_editor.rs:494`), die App-Schreibsperre über
  den Main-Thread-Sprung beim Kamerafenster (`windows.rs:1880`), und
  Einstellungen werden von Rust und Frontend ohne gemeinsame Sperre gelesen
  und geschrieben (`recording_settings.rs:51`).

## 2026-08-20 (Tray-Menü, Geräte in die Einstellungen, Cap-Reste raus)

Das aufgeklappte Menü sah tot aus: nur Text, und die beiden PNG-Symbole waren
schwarz auf dunklem Grund, also unsichtbar. Dazu Geräteauswahl im Menü, die dort
nicht hingehört.

- **Neues `tray_icons.rs`.** Symbole kommen jetzt zur Laufzeit aus SF Symbols und
  werden in der Farbe der aktuellen Darstellung gezeichnet, hell wie dunkel.
  Zwei Fallstricke dabei: `NSGraphicsContext` liefert `nil`, wenn die Bitmap
  nicht vormultipliziertes Alpha hat (deshalb wird beim Auslesen zurückgerechnet),
  und `muda` setzt Menübilder nicht als Template, weshalb die Farbe selbst
  bestimmt werden muss. Beim Systemwechsel hell/dunkel wird das Menü neu gebaut.
- **Statuszeile oben**: "Ready · Studio, Area", "Recording", "Permissions needed".
  Die Berechtigungsabfrage läuft nur, wenn gerade nicht aufgenommen wird.
- **Kürzel stehen an den Zeilen**, gelesen aus dem Hotkey-Store, also immer das,
  was wirklich registriert ist. Ein Kürzel, das die Menü-Schicht nicht schreiben
  kann, fällt weg, statt das ganze Menü scheitern zu lassen.
- **Modus** ist ein eigenes Untermenü mit dem aktuellen Modus im Titel.
- **Kamera, Mikrofon, System-Audio raus aus dem Tray**, jetzt in
  Einstellungen › Geräte unter "In use".
- **Fehlerfunde behoben** (Details in den Commits): Websocket-Server liefen nach
  jedem Editor-Fenster weiter, weil ein Kind-Token statt des Eltern-Tokens
  zurückgegeben wurde; das Kamerafenster konnte sich dauerhaft selbst sperren;
  ein leerer Block versteckte bei jedem Fokuswechsel die Ziel-Auswahl; Cmd+C
  blieb systemweit gekapert, wenn das Overlay zerstört wurde; Hotkey-Fehler
  waren unsichtbar; eine fehlgeschlagene Kopie hing für immer im Fortschritt.
- **Cap-Reste**: `shelf recordings` und `shelf automations` suchten noch unter
  `so.cap.desktop` und fanden deshalb nichts; der Fensterfilter kannte die eigene
  App nicht mehr; `FrameConfiguration::default()` brannte weiter "Cap.so" in
  Exporte. Deep-Link heißt jetzt `shelf://`. CLI-README neu geschrieben, Caps
  Cloud-Skill und die ungenutzte `sentry`-Abhängigkeit entfernt.
- **Caps Cloud-Pakete gelöscht** (database, web-backend, s3, sdk-*, utils, ui und
  weitere, ~300 Dateien). Vorher einzeln geprüft: nur `@cap/ui-solid` wird von der
  Desktop-App importiert. Nebenwirkung: `tailwindcss` und `tailwind-scrollbar`
  mussten in `apps/desktop` selbst deklariert werden.
- Geprüft: `cargo check`, `cargo fmt`, `pnpm typecheck`, `cargo test -p
  cap-recording -p cap-cli-install`, Debug-Build installiert und Menü,
  Untermenü und die neue Geräte-Seite am laufenden Programm angesehen.

## 2026-08-20 (Dock-Icon ließ sich nicht ausschalten)

Der Schalter "Always show dock icon" hatte keine Wirkung: Icon aus, Einstellungen
zu, Icon blieb trotzdem im Dock, bis die App neu startete.

- `lib.rs`: Der `Destroyed`-Zweig für `Settings` und `ModeSelect` sprang auf macOS
  mit `return` heraus, bevor `sync_macos_dock_visibility` am Ende des Arms lief.
  Das `return` stammt aus b1be0164c, als danach noch Reopen-Logik folgte. Heute
  steht dort nur noch die Dock-Synchronisierung, also war es toter Code mit
  Nebenwirkung. Entfernt.
- `permissions.rs`: Zweite Bremse in `sync_macos_dock_visibility`. Der Guard
  `has_visible_panel_window && should_hide_dock` brach genau dann ab, wenn
  ausgeblendet werden sollte. Panels laufen ohnehin unter der Accessory-Policy,
  die `prepare_macos_panel_window` setzt, also ändert das Ausblenden für sie
  nichts. Guard entfernt.
- Geprüft: `cargo check` sauber, Codex-Gegencheck ohne Funde. Branch
  `fix/dock-icon-hide`, noch nicht nach main gemerged, wartet auf Handtest.

## 2026-08-19 (Kürzel wurden gespeichert, aber nicht registriert)

„Area screenshot to text" tat beim Drücken des Kürzels nichts. Der Eintrag
fehlte im gespeicherten Zustand, obwohl er vormittags noch da war.

Ursache war die Kürzel-Seite. Sie schrieb bei **jedem Tastendruck** in den
Speicher, registrierte das Kürzel beim System aber erst beim Klick auf das
Häkchen. Wer die Tasten drückte und das Fenster schloss, hatte ein Kürzel, das
in der Oberfläche gesetzt aussah, auf der Platte stand und trotzdem nichts tat,
bis die App neu startete. Beim Klick auf das rote X wurde der Wert auf
`undefined` gesetzt, was beim Serialisieren wegfällt, der Eintrag verschwand
also ganz.

- **Speichern und Registrieren gehören jetzt zusammen** und passieren erst,
  nachdem das System das Kürzel angenommen hat.
- **Ein gedrücktes Kürzel gilt sofort**, ohne Bestätigungsklick. Das Häkchen
  beendet nur noch den Bearbeitungsmodus, es registriert nicht erneut. Vorher
  scheiterte genau das, weil dasselbe Kürzel kein zweites Mal registrierbar ist.
- **Fehlgeschlagene Registrierung wird sichtbar.** `set_hotkey` gibt den Fehler
  zurück statt ihn mit `.ok()` zu verwerfen, die Zeile stellt den vorherigen
  Zustand wieder her und zeigt „The system refused this shortcut: ...".
- Zwei Aktionen dürfen sich weiter ein Kürzel teilen: eine bereits bestehende
  Registrierung zählt als Erfolg statt als Fehler.
- Beim Start wird jede Registrierung geloggt, fehlgeschlagene als Warnung.

## 2026-08-19 (DMG-Installer trug noch Cap-Namen)

Das Installationsfenster sagte weiter „Drag and Drop **Cap** to the Applications
folder" und „Cap requires macOS 13.1 or greater", obwohl Symbol und App längst
Shelf heißen.

- `assets/dmg-background.png` neu gezeichnet, gleiche Maße, gleiche Farben,
  gleiche Schriftgröße (Kappenhöhe auf das Pixel geprüft), nur mit Shelf im Text.
  Erzeugt aus HTML mit Inter über Chrome headless, nicht generativ, damit die
  Schrift stimmt. 144 dpi wie das Original, sonst zeigt der Finder es falsch.
- `minimumSystemVersion: "13.1"` in der `tauri.conf.json` ergänzt. Im Bundle
  stand vorher Tauris Vorgabe 10.13, das Installationsfenster behauptete 13.1.
  Jetzt sagen beide dasselbe.

Noch offen: Die Windows-Installer-Bilder (`assets/nsis-header.bmp`,
`assets/nsis-sidebar.bmp`) zeigen weiter Caps Logo und Spruch. Fällt nicht auf,
weil Shelf nicht für Windows gebaut wird.

## 2026-08-19 (Menüleisten-App: Hauptfenster entfernt)

Shelf hat kein Hauptfenster mehr. Beim Start erscheint nichts, die Bedienung
läuft komplett über das Menüleisten-Symbol. Es gab die Steuerung vorher doppelt,
im Tray und im Fenster.

- **Fenster ist raus.** `CapWindowId::Main` und `ShowCapWindow::Main` sind
  gelöscht, dazu die Route `/` und der Ordner `new-main/`. Die Teile, die auch
  das Auswahl-Overlay und der Editor nutzen (Kamera- und Mikrofonauswahl,
  Ziel-Kacheln), liegen jetzt unter `src/components/recording/`.
- **Neue Einstellungsseite „Devices".** Auflösung und Bildrate pro Kamera,
  Abtastrate und Kanäle pro Mikrofon. Das gab es vorher nur im Hauptfenster.
- **Tray neu:** „Record Camera Only", „Teleprompter", „Permissions & Tour".
  „Open Main Window" ist weg. Der Teleprompter wird jetzt in Rust gebaut, das
  Fenster wurde vorher aus dem JavaScript des Hauptfensters geöffnet.

Vier Dinge hingen am Fenster und mussten mitgefixt werden, sonst hätte die App
Funktionen verloren:

1. **App beendete sich beim Schließen des letzten Fensters.** Tauri meldet dann
   `ExitRequested`. Vorher fiel das nie auf, weil das Hauptfenster beim
   Schließen nur versteckt wurde und damit immer existierte. `ExitRequested`
   ohne Exit-Code heißt jetzt „bleib laufen". Beenden geht weiter über das Tray
   und über Cmd+Q, beide setzen vorher den Exit-Zustand. Zwei Tests dazu in
   `tests/exit_shutdown.rs`.
2. **Kamera- und Mikrofonauswahl wurde nach jeder Aufnahme geleert.** Der Zweig
   in `handle_recording_end` lief bisher nie, weil das Fenster immer da war.
3. **Prewarm für GPU, Schriften und Screenshot-Editor** hing am Event
   `main-window-ready`. Läuft jetzt direkt beim Start, sonst wäre der erste
   Screenshot wieder rund drei Sekunden langsam gewesen.
4. **Fehler beim Aufnahmestart waren stumm.** Sie liefen als Toast ins
   Hauptfenster. Jetzt kommen sie als Systemmeldung, ebenso ein fehlendes Gerät
   und eine beschädigte Instant-Aufnahme.

Kleinkram: Kamera-Feed in Camera-Only hängt am Kamerafenster statt am
Hauptfenster. Die strikte Rechteprüfung beim Dock-Klick macht jetzt der
Reopen-Handler selbst. Tote Einstellungen entfernt
(`mainWindowRecordingStartBehaviour`, `postDeletionBehaviour`,
`mainWindowPosition`) samt ihrer Oberfläche.

Nicht übernommen: das Ablegen von Videodateien auf dem Fenster. Import läuft
über „Import Media..." im Tray.

## 2026-08-19 (.env dokumentiert, Ordner ersetzbar)

Die letzte undokumentierte Sache im Repo ist weg. In der gitignorierten `.env`
standen nur `NODE_ENV=development` und `VITE_SERVER_URL=https://cap.so`. Kein
Schlüssel, kein Geheimnis. Das `VITE_SERVER_URL` liest kein Code mehr, es war
nur noch als Typ in `apps/desktop/src/vite-env.d.ts` deklariert, seit Caps Cloud
raus ist. Die Deklaration ist entfernt.

- **`.env.example` angelegt** mit den echten Werten plus Kommentaren, warum die
  Datei überhaupt gebraucht wird: `tauri:build`, `with-env` und `cap-setup`
  starten über `dotenv -e .env` und brechen ohne die Datei ab.
- **README** nennt jetzt `cp .env.example .env` als ersten Schritt.
- Damit ist `~/Desktop/Cap` jederzeit löschbar, der Stand steckt vollständig in
  `lou1s19/shelf`.

## 2026-08-18 (Oberfläche aufgeräumt, Screenshots schnell)

Sechs Punkte von Louis, parallel von mehreren Agents umgesetzt. Eigenes
GitHub-Repo angelegt: `lou1s19/shelf`, privat. `origin` zeigt dorthin, der alte
Cap-Fork bleibt als `cap-fork` erhalten. Achtung bei einem späteren Wechsel auf
Open Source: der Fork erbt AGPLv3, eine andere Lizenz ist nicht möglich.

- **Tray-Menü ist jetzt die Steuerzentrale.** Modus direkt mit Haken statt im
  Untermenü, Kamera und Mikrofon als Untermenü, Systemton ankreuzbar, Start und
  Stop als erster Eintrag. Die Geräteliste wird nicht beim Öffnen eingesammelt,
  sondern aus dem ohnehin laufenden `devices-updated`-Event gespiegelt, sonst
  hinge das Menü. Ein Klick aufs Symbol öffnet jetzt immer das Menü, vorher
  stoppte er während einer Aufnahme sofort, wodurch das Menü unbenutzbar war.
  Nicht im Tray: die Feinauswahl welcher Bildschirm oder welches Fenster, dafür
  müsste bei jedem Öffnen die Fensterliste aufgezählt werden.
- **Tray-Symbol** stammt nicht mehr von Cap. Vier Zustände als Silhouette im
  Template-Modus, Quellen liegen unter `icons/src/`. Gestalterisch noch nicht
  überzeugend, siehe `TODO.md`.
- **Einstellungen aufgeteilt.** `general.tsx` von 1470 auf 253 Zeilen. Aufnahme-
  Einstellungen liegen in Recordings, Screenshot-Einstellungen in Screenshots,
  beide Tabs haben Unter-Reiter Library und Settings. In General bleiben nur
  Theme, Dock-Icon, Benachrichtigungen, Update-Kanal. Die Sprungmarken aus dem
  Hauptfenster zu den Qualitätsstufen wurden mitgezogen, sonst wären sie ins
  Leere gelaufen.
- **Shortcuts von 12 auf 9.** Start und Stop teilen sich `toggleRecording`, die
  Entscheidung fällt über `is_recording_active_or_pending()`. Alte Bindungen auf
  `stopRecording` werden einmalig übernommen. Die drei Picker-Kürzel für
  Display, Fenster und Bereich sind raus, das gibt es im Tray. Neu: Warnung bei
  doppelt belegten Kürzeln, vorher feuerten stillschweigend beide Aktionen.
- **Nach einem Screenshot geht nichts mehr auf.** `PostScreenshotBehaviour` hat
  jetzt vier Werte (`openEditor`, `showOverlay`, `saveOnly`, `clipboardOnly`),
  Standard ist `showOverlay`, also nur pinnen. Einmalige Migration zieht ein
  gespeichertes `openEditor` auf `showOverlay`.
- **Pin repariert.** Kartenzahl richtet sich nach der echten Bildschirmhöhe,
  kein unsichtbares Scrollen mehr. Auto-Ausblenden über
  `screenshot_pin_auto_hide_seconds` (`null` = nie, Standard 10), pausiert bei
  Mauskontakt. Ziehen entfernt die Karte, aber nur bei echtem Drop. Das Overlay
  wandert auf den Monitor, auf dem der Screenshot entstand.
- **Zwischenablage terminaltauglich.** Beim Kopieren liegen PNG-Daten,
  `public.file-url`, `NSFilenamesPboardType` und der Pfad als Text auf einem
  Pasteboard-Item. Bildprogramme bekommen das Bild, das Terminal den Pfad.
- **Eindeutige Dateinamen beim Verlassen der App.** Intern heißt jedes Bild
  weiter `original.png`, daran hängen fünf Stellen. Beim Herausziehen und
  Kopieren wird ein Hardlink in `$TMPDIR/Shelf shared files/` mit dem Namen des
  `.cap`-Ordners angelegt, Links älter als 24 Stunden werden aufgeräumt.
- **Screenshot von etwa 3 Sekunden auf 100 bis 200 Millisekunden.** Vier
  Ursachen: unoptimierte Bildbibliotheken im Debug-Bau (`opt-level = 2` für
  Abhängigkeiten in `Cargo.toml`), das Event kam erst nach dem vollen
  PNG-Encode (jetzt erscheint sofort eine JPEG-Vorschau, die Datei folgt), ein
  festes `sleep(1000ms)` für den Overlay-Start (Fenster wird jetzt versteckt
  statt zerstört), und `CompressionType::Default` mit adaptiver Filterung (jetzt
  `Fast` und `FilterType::Up`, Datei rund 10 Prozent größer).
- **CI eingerichtet** (`.github/workflows/ci.yml`): Typecheck, Biome und
  `cargo fmt` auf Linux. Der Rust-Build läuft bewusst NICHT in der CI, er
  bräuchte macOS-Runner zum zehnfachen Minutenpreis und würde das Kontingent
  eines privaten Repos in wenigen Läufen aufbrauchen.

**Bewusst nicht gebaut:** Cmd+C über der Pin-Karte. Das Overlay ist ein
NonActivating-NSPanel und wird erst durch einen Klick Key-Window, beim reinen
Überfahren kommen keine Tastatur-Events an. Die Alternativen wären ein global
registriertes Cmd+C oder das Panel beim Hovern zum Key-Window zu machen, beides
schlimmer als das Problem. Stattdessen sind Copy und Export jetzt echte Knöpfe.

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
