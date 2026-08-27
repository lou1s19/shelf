# Shelf — offene Punkte

Übergabe an die nächste Sitzung. Stand: 2026-08-25.
Verlauf und Begründungen stehen im `CHANGELOG.md`, Projektregeln in `CLAUDE.md`.

## Entschieden am 2026-08-25: Shelf bleibt privat

Louis hat entschieden, Shelf nicht zu veröffentlichen. Die Frage nach Closed
Source ist damit erledigt und braucht nicht neu aufgemacht zu werden: Shelf ist
ein Cap-Fork unter AGPLv3, eine Weitergabe ohne Quelltext wäre nicht erlaubt.
Solange die App nur auf Louis' eigenen Rechnern läuft, entsteht keine Pflicht.

Was daraus folgt und offen ist:

- **Die Website `lou1s19/shelf-website` verlinkt `github.com/lou1s19/shelf` als
  Quellcode** (`src/lib/site.ts`, `repoUrl`). Das Repo ist privat, für Besucher
  ist das ein toter Link. Entweder den Link entfernen oder die Website ganz
  zurückziehen.
- **Der Download bleibt aus** (`download.available = false`). Das muss so
  bleiben, sonst wäre es eine Weitergabe ohne Quelltext.
- Der Punkt "macOS-Runner sind für öffentliche Repos kostenlos" weiter unten ist
  damit hinfällig, der Rust-Build bleibt lokal.

## Zurückgenommen am 2026-08-27: Screen Freeze

Der Versuch, den Hover-Zustand auf den Bereichs-Screenshot zu retten, ist wieder
raus (`git revert 5a931e31a`). Zweimal getestet, zweimal zeigte der Picker den
alten Stand. Das Einfrieren selbst lief nachweislich korrekt, der Fehler steckte
in der Anzeige im Webview.

**Damit weiterhin offen:** Ein Bereichs-Screenshot hält nichts fest, was auf den
Mauszeiger reagiert (Hover, Tooltip, offenes Menü). Ursache ist, dass das
Auswahl-Overlay dem Fenster darunter die Maus nimmt.

**Wer es erneut angeht:** Der Code liegt auf `wip/screen-freeze-restore-fixes`
(`276e7a5e1`). Nicht beim Einfrieren anfangen, das funktionierte. Der Knackpunkt
ist, das Standbild verlässlich anzuzeigen: Der Picker-Webview wird nur versteckt,
nie geschlossen, und behält sein altes Bild. Ein natives Fenster unter dem Picker
wäre vermutlich der robustere Weg als ein `<img>` im Webview.

## Zuerst: noch nicht am lebenden Objekt geprüft

Gebaut und installiert ist alles, die App läuft. **Louis hat den Umbau aber
noch nicht selbst benutzt.** Diese Dinge live prüfen (bauen mit
`scripts/install-shelf.sh`, vorher `pkill -f "Shelf.app/Contents"`):

0. **Kamera und Mikrofon freigeben.** macOS hat beide Rechte für den neuen Build
   noch nicht erteilt, deshalb ist die Geräteliste leer. Im Tray über
   „Permissions & Tour" erteilen, danach in Einstellungen → Devices prüfen, ob
   Kameras und Mikrofone mit ihren Formaten auftauchen. Solange das fehlt, ist
   die neue Seite ungetestet.

1. Fühlt sich der Screenshot jetzt sofort an? Erwartung 100 bis 200 ms bis das
   Pin steht, vorher rund 3 Sekunden.
2. Tray-Menü: Modus, Kamera, Mikrofon, Systemton umstellen. Bleibt das Menü
   flüssig, oder hängt es beim Öffnen?
3. Pin: mehrere Screenshots hintereinander, läuft nichts über den Rand?
   Ausblenden nach 10 Sekunden, Pause bei Mauskontakt, Karte weg nach dem
   Herausziehen.
4. Zwischenablage: Screenshot kopieren, dann einmal in ein Bildprogramm und
   einmal ins Terminal einfügen. Bild hier, Pfad dort.
5. Tray-Symbol in der echten Menüleiste beurteilen, siehe Punkt 2 unten.

**Achtung, falls etwas klemmt:** `open` startet eine laufende App nicht neu.
Immer erst `pkill -f "Shelf.app/Contents"`, sonst testet man die alte Fassung.

## Als Nächstes

1. **Vorschau beim Text-Kürzel.** „Area screenshot to text" kopiert den erkannten
   Text stumm in die Zwischenablage. Es soll kurz ein Fenster zeigen, was kopiert
   wurde, und von selbst wieder verschwinden.
   Betroffen: `apps/desktop/src-tauri/src/recording.rs`
   (`recognize_screenshot_text_to_clipboard`) und ein kleines Fenster analog zum
   Aufnahme-Overlay in `apps/desktop/src-tauri/src/windows.rs`.
   Offen: Wie lange stehen bleiben, wo erscheinen (bei der Maus oder am Rand),
   und was passiert bei langem Text (kürzen oder scrollen).

2. **Tray-Symbol gestalterisch nochmal ansehen.** Das Cap-Symbol ist raus, das
   neue Regal-Motiv liest sich bei Menüleistengröße aber eher wie ein
   Gleichheitszeichen, und der Punkt der Instant-Variante wirkt wie ein Fleck.
   Technisch in Ordnung (Template-Modus, vier Zustände, Quellen unter
   `apps/desktop/src-tauri/icons/src/`), nur nicht unverwechselbar.
   Offen: erst live in der Menüleiste ansehen, dann entscheiden, ob eine andere
   Richtung her muss. Die alten Cap-Symbole stecken im Git-Verlauf, falls du
   vergleichen willst (Commit davor: `1c4809e89`).

3. **Kleinkram aus dem Geschwindigkeits-Umbau.** Alles gemessen, alles gering:
   - `apps/desktop/src/routes/target-select-overlay.tsx:1264`: zusätzliche 50 ms
     `setTimeout` nur im Bereichs-Weg. Rust wartet danach ohnehin, kann weg.
   - `crates/recording/src/screenshot.rs:63-105`: skalare BGRA-nach-RGB-Schleife,
     rund 50 ms für 3,7 Megapixel. Als Chunk-Iterator oder mit `rayon` deutlich
     schneller.
   - „Camera Only" als Aufnahmeziel fehlt im Tray-Menü, im Hauptfenster gibt es
     das. Die Feinauswahl welcher Bildschirm oder welches Fenster fehlt bewusst,
     dafür müsste bei jedem Öffnen die Fensterliste aufgezählt werden.

## Danach

3b. **Windows-Installer-Bilder tragen noch Cap.** `assets/nsis-header.bmp` und
   `assets/nsis-sidebar.bmp` zeigen Caps Logo und „Beautiful screen recordings,
   owned by you." Nur sichtbar, wenn jemand für Windows baut, was gerade
   niemand tut. Beim Erneuern denselben Weg wie beim DMG-Hintergrund nehmen:
   selbst zeichnen statt generieren, sonst wird der Text falsch.


4. **Eigener Update-Weg.** `apps/desktop/src-tauri/src/updates.rs` ist ein
   Platzhalter, der immer „keine Aktualisierung" meldet. Caps Endpunkt ist raus,
   weil er Shelf durch Cap ersetzt hätte. Nötig wäre ein eigener Release-Feed
   plus Signierschlüssel, oder die Funktion ganz aus den Einstellungen nehmen.

5. **Namen im Paketinneren.** Die mitgelieferten Hilfsprogramme heißen weiter
   `cap-cli`, `cap-exporter`, `cap-muxer`, die Rust-Crates `cap-*`, die
   npm-Pakete `@cap/*`, und die Projektendung ist `.cap`. Im Betrieb sichtbar
   ist davon nichts, das Deep-Link-Schema heißt seit 2026-08-20 `shelf://`.
   Das Umbenennen der Endung berührt viele Stellen und braucht eine Migration
   für bestehende Aufnahmen.

6. **Sprachmodelle für die Untertitel** kommen von
   `github.com/CapSoftware/transcription-models`. Funktioniert, hängt aber an
   einem fremden Repo. Eigene Ablage oder ein anderer Anbieter wäre sauberer.

7. **Mehrsprachigkeit fehlt.** Alle Texte stehen fest im Code auf Englisch.

8. **CI deckt den Rust-Build nicht ab.** `.github/workflows/ci.yml` prüft nur
   Typecheck, Biome und `cargo fmt` auf Linux. Der eigentliche Build bräuchte
   macOS-Runner zum zehnfachen Minutenpreis und würde das Kontingent eines
   privaten Repos in wenigen Läufen aufbrauchen. Sobald das Repo öffentlich ist,
   sind macOS-Runner kostenlos, dann `cargo check -p cap-desktop` ergänzen.

## Erledigt am 2026-08-19 (nicht nochmal anfassen)

Shelf ist eine reine Menüleisten-App, das Hauptfenster ist gelöscht. Details im
`CHANGELOG.md`. Kurz: Route `/` und `new-main/` weg, geteilte Bausteine unter
`src/components/recording/`, neue Einstellungsseite „Devices" für die
Geräte-Formate, Tray um „Record Camera Only", „Teleprompter" und
„Permissions & Tour" ergänzt. Vier Kopplungen ans Fenster waren mitzufixen:
Beenden beim Schließen des letzten Fensters, geleerte Geräteauswahl nach jeder
Aufnahme, Prewarm am Fenster-Event und stumme Startfehler.


`.env.example` liegt jetzt im Wurzelverzeichnis und ist committet. In der `.env`
standen nur `NODE_ENV=development` und ein `VITE_SERVER_URL=https://cap.so`, das
kein Code mehr liest. Keine Schlüssel, kein Geheimnis. Die tote Deklaration von
`VITE_SERVER_URL` ist aus `apps/desktop/src/vite-env.d.ts` raus. Gebraucht wird
die `.env` nur noch, weil `tauri:build`, `with-env` und `cap-setup` sie per
`dotenv -e .env` einlesen und ohne Datei abbrechen.
**Damit ist der lokale Ordner `~/Desktop/Cap` ersetzbar**, alles steckt in
`lou1s19/shelf`. Nach einem frischen Klon: `cp .env.example .env`.

## Erledigt am 2026-08-18 (nicht nochmal anfassen)

Die sechs Punkte von Louis (Tray-Menü, Einstellungen sortiert, Shortcuts
zusammengefasst, CLI bleibt, kein Menü nach dem Screenshot, Pin bleibt im Bild)
plus Geschwindigkeit, eindeutige Dateinamen, Zwischenablage im Terminal und das
Aufräumen der `cargo check`-Warnungen. Details im `CHANGELOG.md`.

## Wie gebaut und getestet wird

```sh
scripts/install-shelf.sh          # Debug-Bau nach /Applications, ~2 Minuten
scripts/install-shelf.sh release  # optimiert, ~40 Minuten
```

Beide tragen dieselbe Kennung `de.shelf.desktop` und dieselbe Signatur, deshalb
bleiben die macOS-Berechtigungen erhalten. Zum Ausprobieren immer den
Debug-Bau nehmen, der Unterschied fällt nur beim Video-Export auf.

## Fallen, die schon Zeit gekostet haben

- **Alte Instanz.** `open` startet eine laufende App nicht neu, sondern holt sie
  nach vorn. Nach jedem Bau erst `pkill -f "Shelf.app/Contents"`, sonst testet
  man die alte Fassung. Genau daran sind wir einmal hängen geblieben.
- **DMG bleibt gemountet.** Der Bau erzeugt neben der App auch ein DMG und
  hängt es ein. `open -a Shelf` startet dann die Kopie von `/Volumes/dmg.XXXX/`,
  und die hat keine Systemrechte, also scheitert die Bildschirmaufnahme mit
  „TCCs abgelehnt". Zum Testen den vollen Pfad nehmen:
  `/Applications/Shelf.app/Contents/MacOS/Shelf`, und mit `hdiutil detach`
  aufräumen. Hat schon einmal eine falsche Fehlersuche ausgelöst.
- **iCloud im Repo-Ordner.** `~/Desktop` wird synchronisiert und hängt den
  Dateien laufend Zusatzinfos an, die `codesign` ablehnt. Deshalb wird mit
  `ditto --noextattr` nach `/Applications` kopiert und erst dort signiert.
- **Fremde Signatur in den Bibliotheken.** Die mitgelieferten Medien-Bibliotheken
  waren mit einem anderen Entwickler-Team signiert, dadurch startete die App gar
  nicht. Alle Bibliotheken, das Framework und die Hilfsprogramme müssen von innen
  nach außen neu signiert werden. Das Skript macht das.
- **Vorgabewerte greifen nur, wenn nichts gespeichert ist.** Wurde eine
  Einstellung einmal geschrieben, gewinnt sie gegen jede neue Vorgabe. Beim
  Testen also entweder den Store unter
  `~/Library/Application Support/de.shelf.desktop/store` anfassen oder die
  Einstellung in der App umstellen.
- **Tauri-Fensterspeicher.** `vendor/tauri-runtime-wry` ist gepatcht, weil eine
  hängende Borrow-Sperre die App beim Öffnen des Overlays getötet hat. Nicht
  entfernen, ohne zu prüfen, ob eine neuere Tauri-Version das behoben hat.
