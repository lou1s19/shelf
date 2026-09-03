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

## 2026-09-03 (Mehrere Bilder untereinander in die Zwischenablage)

Louis: mehrere Shelf-Bilder nacheinander kopieren soll sie untereinander in eine
Zwischenablage legen. Kopiert man ein zweites Bild direkt nach dem ersten, liegt
jetzt ein Bild aus beiden auf der Zwischenablage, von oben nach unten, jeweils
mittig auf der Breite des breitesten, mit 12 px Abstand dazwischen.

- Neues Modul `apps/desktop/src-tauri/src/clipboard_stack.rs`. Eine Serie läuft
  weiter, solange die Zwischenablage noch das hält, was die App zuletzt
  geschrieben hat (auf macOS über `NSPasteboard changeCount`), und der letzte
  Kopiervorgang keine 10 Sekunden her ist. Sonst fängt sie von vorne an.
- `write_screenshot_to_clipboard` kopiert weiter genau ein Bild und beendet eine
  laufende Serie. Neu daneben: `write_screenshot_to_clipboard_stacked`, das die
  Serie fortsetzt und die Anzahl zurückgibt. Der Tauri-Befehl
  `copy_screenshot_to_clipboard` gibt diese Anzahl jetzt zurück.
- Die Shelf-Karte zeigt beim Häkchen „N images", sobald mehr als eins drauf ist.
- Das automatische Kopieren nach einem Screenshot (`ClipboardOnly`) stapelt
  bewusst nicht: dort ist jeder Screenshot eine eigene Absicht.
- Neu: `scripts/dev-shelf.sh` (start/reload/stop/status) startet die Debug-App
  neben der installierten Shelf.app, damit Änderungen ohne kompletten
  Neubau-und-Installieren-Lauf ausprobiert werden können.
- **Noch nicht gebaut und nicht getestet**, auf Wunsch von Louis erst eingebaut.

## 2026-08-28 (Kein Einfrieren mehr beim Schließen eines Fensters)

Louis: „manchmal hängt sich die App auf", dazu ein macOS-Hang-Bericht. Der Bericht
und `panics.log` zeigen die Kette: Screenshot-Editor schließen → `on_window_close`
→ `win_borrow_mut` → **BorrowMutError** auf dem Main-Thread mitten in der
Event-Schleife → der Panic wickelt sich bis `main.rs` zurück → die Tokio-Runtime
wird abgebaut und wartet ewig auf einen Task, der im `fake_window`-Listener in
`WebviewWindow::outer_position()` auf Antwort der toten Event-Schleife wartet.
Ergebnis: Beachball, 45 Sekunden gemessen, nur per Abschuss zu beenden.

- **Schreibzugriffe auf den Fensterspeicher können nicht mehr panicken.**
  Der Fork sicherte bisher nur Lesezugriffe gegen die hängende `RefCell`-Sperre ab
  (`win_borrow`). `win_borrow_mut` rief weiter direkt `borrow_mut()` auf und war
  damit die eine Stelle, an der der bekannte Tauri-Fehler die App noch killen
  konnte. Jetzt gilt derselbe Weg wie beim Lesen: hängende Sperre ohne echten
  Zugriff → direkt schreiben, echter Zugriff → Schreibvorgang auslassen statt
  abstürzen. Jeder Aufruf nennt seinen Kontext, damit die Logzeile verrät, welcher.
- **Fenster-Änderungen gehen nicht mehr verloren.** Aus der Warteschlange für
  aufgeschobene Fenster-Einfügungen ist eine für alle drei Lebenszyklus-Schritte
  geworden (`Insert`, `Remove`, `Detach`), in der Reihenfolge, in der sie anfielen.
  Ist der Speicher gerade belegt, wird die Änderung geparkt und im nächsten
  Durchlauf der Event-Schleife angewandt, statt still verworfen zu werden.
- **Der Prozess kann nicht mehr am Runtime-Abbau hängen bleiben.** `main.rs` fängt
  einen Panic aus der Event-Schleife ab und beendet den Prozess sofort; ein
  normales Ende bekommt für den Runtime-Abbau 2 Sekunden Frist statt unbegrenzt.
  Das ist die Sicherheitsleine: Selbst ein anderer Absturz an dieser Stelle endet
  ab jetzt als Absturz, nicht als eingefrorenes Fenster.
- **Nebenbei geprüft und gehärtet:** Die Erkennung „Sperre hängt" zählt den
  eigenen Zugriff jetzt vor der Prüfung mit, damit zwischen Prüfung und Zugriff
  niemand mehr dazwischenrutschen kann (Fund aus dem Codex-Gegencheck).
- Geprüft: `cargo check -p cap-desktop` ohne Warnungen, `cargo fmt --check`,
  `cargo test -p cap-desktop --lib` 127 Tests grün, Codex-Gegencheck.
- **Falls der Testlauf mal „crate `cocoa` required to be available in rlib format"
  sagt:** Das ist ein kaputter `target/`-Stand, kein Code-Fehler. `cargo clean -p
  cocoa -p metal -p wgpu-hal` reicht, ein volles `cargo clean` ist nicht nötig.

## 2026-08-27 (Kürzel für den letzten Bereich, hält den Hover)

Louis: „mach das es klappt, mit hover ohne verzögerung, lass den mauszeiger auf
der stelle liegen wo es war, ohne frieze oder so." Neue Aktion
`ScreenshotLastArea` in den Tastenkürzeln: „Repeat last area (keeps hover)".

**Warum das geht, wo drei Anläufe scheiterten.** Der Mauszeiger blieb immer
liegen, das war nie das Problem. Der Hover geht verloren, weil das Auswahl-Overlay
über das Fenster darunter rutscht und macOS diesem meldet, die Maus sei weg. Und
sobald man zum Aufziehen des Bereichs die Maus bewegt, wäre der Hover ohnehin
fort, ganz ohne Shelf. Deshalb ließ er sich bisher nur als Standbild retten, also
per Freeze, was Louis nicht wollte.

Der Ausweg ist, das Overlay wegzulassen: Dieses Kürzel nimmt den zuletzt
gewählten Bereich sofort auf. Nichts legt sich über den Zeiger, also erfährt die
App darunter nie, dass die Maus weg ist, und der Button bleibt hell. Kein
Standbild, keine Verzögerung, kein Webview, der ein altes Bild halten könnte:
genau die drei Stellen, an denen es vorher schiefging, existieren hier nicht.

- **Der Bereich hat ein eigenes Feld,** `last_screenshot_area`. Der erste Anlauf
  las `target`, und der Codex-Gegencheck zeigte, dass das nie funktioniert hätte:
  Der Bereichs-Screenshot schreibt seinen Bereich dort gar nicht hinein, und das
  Feld wird von jeder Video-Auswahl (Bildschirm, Fenster, Kamera) überschrieben.
  Das Kürzel hätte also „No Area Yet" gemeldet oder einen alten Aufnahme-Bereich
  genommen. Geschrieben wird jetzt in `take_screenshot`, also unabhängig davon,
  über welchen Weg der Screenshot ausgelöst wurde.
- **Kein Bereich gewählt:** Systemmeldung „No Area Yet", zusätzlich eine
  Log-Zeile, weil `send_notification` bei abgeschalteten Systemmeldungen still
  zurückkehrt und das Kürzel sonst gar nichts täte.
- **Kein Standard-Kürzel** vergeben, das legt Louis in den Einstellungen fest.
- Eine Log-Zeile nennt den benutzten Bereich, damit ein Fehlschlag am Log ablesbar
  ist statt durch Raten.

**Noch nicht am lebenden Objekt geprüft.** Louis hat den Push freigegeben, ohne
das Kürzel zu testen.

## 2026-08-27 (Screen Freeze wieder ausgebaut)

Der Versuch, den Hover-Zustand auf den Bereichs-Screenshot zu retten, ist
zurückgenommen. Louis hat ihn zweimal getestet, beide Male zeigte der Picker den
Stand von vor dem Tab-Wechsel. Der Rückbau ist ein `git revert` von `5a931e31a`.

**Was das Feature tat:** Beim Drücken von Cmd+B wurde der Bildschirm unter der
Maus sofort aufgenommen, bevor Shelfs Auswahl-Overlay sichtbar wurde, als JPEG in
den Temp-Ordner geschrieben, im Picker angezeigt und die Auswahl daraus
geschnitten. Grund: Das Overlay nimmt dem Fenster darunter die Maus, damit ist
ein hervorgehobener Button wieder normal und ein Tooltip ganz weg.

**Warum es raus ist.** Der Rust-Teil arbeitete am Ende nachweislich korrekt. Das
Log vom 27.08. zeigt zwei aufeinanderfolgende Aufnahmen, beide mit frischem
Einfrieren (409 ms und 561 ms), dazwischen ein fertiger Screenshot. Trotzdem sah
Louis den alten Stand. Der Fehler steckt also nicht im Einfrieren, sondern in der
Anzeige: Der Picker ist ein Webview, das nur versteckt und nie geschlossen wird,
und behält damit sein bisheriges `<img>`. Ein Cache-Buster an der URL hat nicht
gereicht. Weiter verfolgt wurde das nicht.

**Aufgehoben, nicht gelöscht.** Der Stand liegt auf `wip/screen-freeze-restore-fixes`
(`276e7a5e1`), inklusive der drei zuletzt eingearbeiteten Codex-Funde. Wer es
wieder aufgreift, fängt bei der Anzeige an, nicht beim Einfrieren.

**Was damit wieder offen ist:** Louis' ursprüngliche Bitte. Ein Bereichs-Screenshot
kann nichts festhalten, was auf den Mauszeiger reagiert: keinen Hover, keinen
Tooltip, kein geöffnetes Menü. Das ist keine Nachlässigkeit, sondern die Folge
davon, dass das Auswahl-Overlay den Bildschirm verändert, den es aufnehmen soll.
Wer es erneut angeht, braucht einen Weg, das Standbild verlässlich anzuzeigen,
zum Beispiel ein natives Fenster unter dem Picker statt eines Bildes im Webview.

**Ebenfalls zurückgenommen,** weil es nur zum Freeze gehörte: das Wegblenden und
Wiedereinblenden eigener Overlays vor dem Einfrieren samt Merkliste.

## 2026-08-25 (Hovern wählte den falschen Bildschirm aus)

Louis: beim Hovern über einen Bildschirm wählte die Auswahl den anderen aus,
nach einem Neustart der App ging es wieder. Branch
`fix/overlay-stale-display-geometry`.

- **Ursache:** Die Auswahl-Overlays werden zwischen zwei Aufnahmen nur versteckt,
  nie geschlossen. Position und Größe bekommt so ein Fenster genau einmal, beim
  Erzeugen, aus den Grenzen seines Displays. Ändert sich danach die Anordnung
  (Monitor an- oder abgesteckt, Auflösung oder Skalierung geändert, macOS sortiert
  die Displays um), liegt das Overlay von Display A über Display B. Geklickt wird
  dann Display A, obwohl B unter der Maus liegt. Der Neustart half, weil dabei
  alle Fenster neu erzeugt werden.
- **Fix:** `sync_overlay_to_display` in `windows.rs` setzt vor jedem Anzeigen die
  aktuellen Grenzen des Displays neu. Genau dasselbe hatte das Screenshot-Regal
  (`RecordingsOverlay`) schon, dem Auswahl-Overlay fehlte es.
- **Zweiter, kleinerer Anteil:** `isHoveredDisplay` steht in der URL des Fensters
  und ist bei einem wiederverwendeten Overlay von der allerersten Öffnung. Im
  Bereichs-Modus entschied dieser eingefrorene Wert, welcher Bildschirm den
  Auswahlrahmen zeigt, solange noch kein Cursor-Ereignis eingetroffen war. Rust
  schickt jetzt eine frische Position raus, bevor überhaupt ein Fenster sichtbar
  wird.
- **Codex-Gegencheck:** ein echter Fund, eingearbeitet. Der Windows-Zweig setzte
  Größe und Position ohne die Verzögerung und Nachkontrolle, die der Erzeugungsweg
  hat; bei gemischter Skalierung deckt das Overlay dann nur einen Teil des
  Bildschirms ab.
- **Nebenbei:** `pnpm exec biome check` ist wieder sauber. `vendor/` wird nicht
  mehr mitgeprüft (fremder Code), und `capabilities/default.json` ist formatiert.
- Geprüft: `cargo check -p cap-desktop` ohne Warnungen, `cargo test -p cap-desktop`
  141 Tests grün, Typecheck, Biome, `cargo fmt --check`.

## 2026-08-25 (Die Test-Suite lief nie durch)

Beim Prüfen der App fiel auf, dass `cargo test --workspace` gar nicht bis zum
Ende kommt. Ursache waren Reste aus dem Löschen von Caps Cloud-Paketen: die
Testdatei der CLI blieb stehen, die geprüften Befehle sind weg.

- **Zwei Tests hingen für immer.** `auth_status_verifies_agent_credentials_with_the_server`
  und `mcp_cancellation_stops_wait_polling` starten einen Mini-Server auf
  127.0.0.1 und warten in `accept()` auf eine Anfrage der CLI. Die kommt nie,
  weil `auth` und `mcp` gelöscht sind. Kein Timeout, der Lauf steht.
- **15 Tests entfernt**, alle für Befehle, die es nicht mehr gibt (`auth`,
  `caps`, `account`, `agents`, `upload`, `mcp`, S3). Einer davon war grün, aber
  aus dem falschen Grund: `sharing_requires_one_visibility_flag` erwartet
  Exit-Code 2 für eine falsche Flag-Kombination, und clap liefert dieselbe 2
  auch für einen unbekannten Befehl. Der Test hat nichts geprüft.
- **Vier Tests auf den Ist-Zustand umgestellt**: die Befehlslisten in
  `help_succeeds_and_lists_commands` und `subcommand_help_succeeds`, der Name
  im Startbildschirm (`shelf` statt `cap`) und das Guide-Manifest.
- Ergebnis: 42 grün, 0 rot, kein Hänger. Vorher 38 grün, 17 rot, 2 endlos.
- **Zwei Doctests in `crates/utils`** waren nie kompilierbar (fehlender Import,
  freischwebende Variablen; der zweite Block war überhaupt kein Rust, sondern
  Format-Beispiele). Erster jetzt ein echtes Beispiel, zweiter als `text`
  ausgezeichnet.
- **`test_needs_update` in `cap-rendering-skia`** prüfte nach `prepare()`, dass
  kein Update nötig ist. `needs_update` vergleicht aber gegen `last_rendered_*`,
  und die schreibt nur `record()`; der Kommentar am Ende von `prepare` sagt das
  ausdrücklich. Der Test stammt aus der Zeit, als gegen `current_*` verglichen
  wurde. Er läuft jetzt den echten Zyklus und prüft zusätzlich, dass `record`
  ein Bild liefert.
- **Codex-Gegencheck:** meldete einen Build-Fehler wegen `unused_must_use`.
  Nachgemessen, stimmt nicht, `Option` ist nicht `#[must_use]`, nur `Result`;
  der Test kompilierte und lief. Der Vorschlag, das Ergebnis zu prüfen, macht
  den Test trotzdem besser und ist übernommen.

- **Doctests von `cap-audio` abgeschaltet.** Seit Edition 2024 fasst rustdoc alle
  Doctests einer Kiste zu einem Programm zusammen und startet es, auch wenn jedes
  Beispiel `no_run` ist. Dieses Programm liegt in einem Temp-Ordner, und die
  mitgelieferten ffmpeg-Bibliotheken sind als `@executable_path/../Frameworks/...`
  eingebunden, also findet dyld sie dort nicht. `cargo test --workspace` endete
  deshalb mit einem dyld-Fehler statt mit einem Ergebnis. Das einzige Beispiel der
  Kiste ist `no_run` und lief ohnehin nie. Begründung steht in der `Cargo.toml`.

Offen und bewusst nicht angefasst: `hardware_instant_recording` und
`hardware_studio_recording` nehmen wirklich den Bildschirm auf und vergleichen
die Videolänge mit der Aufnahmedauer. Im ersten Lauf schlugen sie fehl (29 Bilder
in 6 Sekunden, Video 1,9 statt 6 Sekunden), im zweiten Lauf mit unverändertem
Code waren sie grün. ScreenCaptureKit liefert Bilder nur bei Änderung, bei
stillstehendem Bildschirm kommt zu wenig an. Kein Fehler in der App, aber das
Ergebnis hängt davon ab, was während des Laufs auf dem Bildschirm passiert.

## 2026-08-21 (App hing sich beim Fenster-Erzeugen auf)

Shelf blieb im Betrieb stehen: keine Reaktion mehr, ein Kern dauerhaft auf
100 Prozent, nur noch abschießbar. Ausgelöst hat es ein Screenshot, der per
Drag-and-Drop zurück in die App gezogen wurde. Der Import öffnet dafür ein
Editor-Fenster, und genau beim Eintragen dieses Fensters blieb die App hängen.

- **Ursache: eine Endlosschleife ohne Ausstieg in `vendor/tauri-runtime-wry`.**
  `Message::CreateWindow` trug das fertige Fenster so in den Fensterspeicher ein:
  `loop { if let Ok(mut w) = windows.0.try_borrow_mut() { .. break } }`. Kein
  Zeitlimit, keine Pause. War die Sperre in dem Moment vergeben, drehte die
  Schleife auf dem Haupt-Thread, bis die App abgeschossen wurde. Das Erzeugen
  eines Fensters fährt auf macOS einen verschachtelten Event-Loop, der
  Fensterspeicher kann also weiter oben im Stack noch gelesen werden; der Borrow
  wird dann erst frei, wenn diese Schleife endet, und die endet nie.
- **Beleg, nicht Vermutung.** Der Bericht
  `/Library/Logs/DiagnosticReports/Shelf_2026-08-21-122500_*.cpu_resource.diag`
  zeigt 90 Sekunden CPU in 90 Sekunden, "unresponsive for 84 seconds", und einen
  Stack, der in `handle_event_loop` direkt hinter dem Sprung nach
  `handle_user_message` steht. Der einzige tiefere Frame ist `Arc::deref`, mit
  9 von 29 Proben. Das passt genau auf `windows.0` in dieser Schleife, denn
  `windows` ist ein `Arc<WindowsStore>` und wird pro Durchlauf dereferenziert.
  Der Zeitpunkt deckt sich auf die Sekunde mit `Starting image import` im
  App-Log. Die Schleife war die einzige Endlosschleife in `handle_user_message`.
- **Fix: parken statt warten.** Neu sind `win_try_borrow_mut` (nimmt die
  Schreibsperre oder gibt auf, statt zu paniken, und zählt den Schreiber weiter
  mit, damit `win_borrow` es nicht fälschlich für eine hängende Flagge hält),
  `win_insert_window` und `win_flush_pending_inserts`. Ist der Speicher belegt,
  wandert das Fenster in eine Warteschlange und wird beim nächsten Durchlauf
  eingetragen. Kosten: ein Event-Loop-Durchlauf. Hängen kann es nicht mehr.
  Geleert wird die Warteschlange am Anfang von `handle_event_loop` und von
  `handle_user_message`.
- **Bekannte Einschränkung des Fixes.** Ein geparktes Fenster gilt für Tauri
  schon als erzeugt, steht aber noch nicht im Speicher. Kommt im selben Durchlauf
  sofort ein `show` oder ein Getter dafür, findet der die ID nicht und läuft ins
  Leere. Das trifft nur den Ausnahmefall, in dem der Speicher gerade wirklich
  benutzt wird, und es wird als Warnung geloggt. Sauber wäre, das Erzeugen des
  Fensters nicht mehr unter fremdem Borrow laufen zu lassen; das ist ein Umbau
  von Tauris Fensterverwaltung und steht hier bewusst nicht drin.
- **Nebenbei mitgenommen:** verdrängte Fenster werden erst nach Freigabe der
  Sperre fallen gelassen. Das Aufräumen eines Fensters kann selbst wieder einen
  verschachtelten Event-Loop fahren, und das während gehaltener Sperre wäre
  derselbe Fehler nochmal.
- **Auch repariert: das Install-Skript baut fehlende Sidecars selbst.** In einem
  frischen Klon fehlen `cap-muxer`, `cap-cli` und `cap-exporter`, und Tauri merkt
  das erst nach einigen Minuten Rust-Build (`resource path
  binaries/cap-muxer-aarch64-apple-darwin doesn't exist`). Fehlen sie, ruft
  `install-shelf.sh` jetzt vorher `scripts/build-desktop-binaries.sh` auf.
- **Auch repariert: `scripts/install-shelf.sh release` lief nie.** macOS liefert
  bash 3.2, dort zählt ein leeres Array unter `set -u` als nicht gesetzt, und
  `BUILD_FLAGS=()` für den Release-Build ließ das Skript sofort mit
  `BUILD_FLAGS[@]: unbound variable` aussteigen. Jetzt wird das Array nur
  expandiert, wenn es Einträge hat.

## 2026-08-20 (Speichern und Export bei Screenshots repariert)

Drei Fehler, die zusammen dafür sorgten, dass ein Screenshot nur noch über
Kopieren oder Herausziehen aus der App kam.

- **Der Speichern-Dialog im Screenshot-Editor erschien nie.** Shelf ist eine
  Menüleisten-App und beim Klick auf Speichern oft nicht die aktive Anwendung.
  macOS legt das Bedienfeld dann nicht auf den Bildschirm: gemessen am laufenden
  Programm existierte weder ein Fenster noch ein Sheet, das Versprechen im
  Frontend wurde nie eingelöst, `isExporting` blieb dauerhaft `true`. Danach war
  auch **Kopieren** tot, weil `exportImage` gleich am Anfang aussteigt, wenn ein
  Export zu laufen scheint. Genau das war der gemeldete Fehler. Nach dem Fix
  hängt das Sheet sichtbar am Editorfenster.
  `save_file_dialog` in `lib.rs` aktiviert die App vorher, nennt das aufrufende
  Fenster als Elternfenster und weitet den Datei-Scope auf die gewählte Datei aus
  (das machte vorher das Dialog-Plugin). `export.rs` macht dasselbe für den
  Video-Export, und `useScreenshotExport.ts` benutzt jetzt dieses Kommando statt
  `save()` aus dem Dialog-Plugin.
- **Der Export-Knopf auf der Pin-Karte tat nichts.** Bei Screenshots zeigt
  `media.path` auf `<Name>.cap/original.png`, `get_recording_meta` erwartet aber
  den `.cap`-Ordner, weil dort `recording-meta.json` liegt. Die Abfrage schlug
  jedes Mal fehl, und die Mutation warf „Recording metadata not available",
  bevor irgendetwas passierte. Der Fehler war unsichtbar: kein Toast, nur die
  Fortschrittskarte, die nach zwei Sekunden wieder verschwand.
  `recordings-overlay.tsx` löst den Bundle-Pfad jetzt selbst auf, nimmt bei
  fehlenden Metadaten den Bundle-Namen als Vorschlag und loggt Fehler.
- **Die Vorschau im Screenshot-Editor fror ein.** Ein Editor-Bild ist rund 11 MB.
  Zwei davon kurz hintereinander sprengen die Puffer des Loopback-Sockets, macOS
  antwortet mit `ENOBUFS` (os error 55). Das ist Gegendruck, wurde aber wie ein
  Verbindungsabbruch behandelt und der Socket geschlossen, siehe
  `shelf.log.2026-08-20`, 13:09:37. Danach blieb das letzte Bild stehen, die
  Vorschau folgte den Änderungen nicht mehr, und ein Export konnte mit
  „Preview is still updating" abbrechen. `frame_ws.rs` wiederholt solche Sendungen
  jetzt (die ungeschriebenen Bytes bleiben in Tungstenites Puffer, ein erneutes
  `flush` setzt dasselbe Bild fort), und `screenshot-editor/context.tsx` baut die
  Verbindung neu auf, wenn sie doch abreißt.

## 2026-08-20 (Tray-Menü, Geräte in die Einstellungen, Cap-Reste raus)

Das aufgeklappte Menü sah tot aus: nur Text, und die beiden PNG-Symbole waren
schwarz auf dunklem Grund, also unsichtbar. Dazu Geräteauswahl im Menü, die dort
nicht hingehört.

- **Neues Menüleisten-Symbol, eins pro Modus.** Gezeichnet von
  `apps/desktop/src-tauri/icons/make-tray-icons.py`: ein Bildschirm bleibt
  konstant, was darin sitzt, sagt den Modus. Studio leer, Instant ein Blitz,
  Screenshot ein gefüllter Block, beim Aufnehmen ein Punkt. Ausgewählt aus zwölf
  Entwürfen, die vorher als Kontaktbogen in echter Menüleistengröße nebeneinander
  lagen, hell und dunkel. Die Linux-SVGs haben dieselbe Geometrie bekommen.
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
