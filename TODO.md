# Shelf — offene Punkte

Übergabe an die nächste Sitzung. Stand: 2026-08-18.
Verlauf und Begründungen stehen im `CHANGELOG.md`, Projektregeln in `CLAUDE.md`.

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
   Richtung her muss. Die alten Symbole liegen als `*.png.bak` daneben.

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

3. **Eigener Update-Weg.** `apps/desktop/src-tauri/src/updates.rs` ist ein
   Platzhalter, der immer „keine Aktualisierung" meldet. Caps Endpunkt ist raus,
   weil er Shelf durch Cap ersetzt hätte. Nötig wäre ein eigener Release-Feed
   plus Signierschlüssel, oder die Funktion ganz aus den Einstellungen nehmen.

4. **Namen im Paketinneren.** Die mitgelieferten Hilfsprogramme heißen weiter
   `cap-cli`, `cap-exporter`, `cap-muxer`, die Projektendung ist `.cap` und das
   Deep-Link-Schema `cap-desktop`. Im Betrieb sichtbar ist davon nichts. Das
   Umbenennen der Endung berührt viele Stellen und braucht eine Migration für
   bestehende Aufnahmen.

5. **Sprachmodelle für die Untertitel** kommen von
   `github.com/CapSoftware/transcription-models`. Funktioniert, hängt aber an
   einem fremden Repo. Eigene Ablage oder ein anderer Anbieter wäre sauberer.

6. **Mehrsprachigkeit fehlt.** Alle Texte stehen fest im Code auf Englisch.

7. **`.env.example` fehlt.** Für den Desktop-Build reicht eine kleine `.env` im
   Repo-Root, deren Variablen sind nicht dokumentiert.

8. **Warnungen aufräumen.** `cargo check` meldet acht ungenutzte Variablen in
   `recovery.rs` und `recording.rs`, Reste aus dem Ausbau der Telemetrie. Die CI
   läuft mit `-D warnings` und würde daran scheitern.

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
