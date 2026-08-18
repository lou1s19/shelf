# Shelf — offene Punkte

Übergabe an die nächste Sitzung. Stand: 2026-08-18, abends.
Verlauf und Begründungen stehen im `CHANGELOG.md`, Projektregeln in `CLAUDE.md`.

## Zuerst: noch nicht am lebenden Objekt geprüft

Der ganze Umbau vom 18.08. ist gebaut, übersetzt sauber und ist committet, aber
**Louis hat ihn noch nicht in Betrieb gesehen**. Vor allem anderen einmal bauen
(`scripts/install-shelf.sh`, vorher `pkill -f "Shelf.app/Contents"`) und diese
fünf Dinge live prüfen:

1. Fühlt sich der Screenshot jetzt sofort an? Erwartung 100 bis 200 ms bis das
   Pin steht, vorher rund 3 Sekunden.
2. Tray-Menü: Modus, Kamera, Mikrofon, Systemton umstellen. Bleibt das Menü
   flüssig, oder hängt es beim Öffnen?
3. Pin: mehrere Screenshots hintereinander, läuft nichts über den Rand?
   Ausblenden nach 10 Sekunden, Pause bei Mauskontakt, Karte weg nach dem
   Herausziehen.
4. Zwischenablage: Screenshot kopieren, dann einmal in ein Bildprogramm und
   einmal ins Terminal einfügen. Bild hier, Pfad dort.
5. Tray-Symbol in der echten Menüleiste beurteilen, siehe Punkt 3 unten.

**Achtung, falls etwas klemmt:** `open` startet eine laufende App nicht neu.
Immer erst `pkill -f "Shelf.app/Contents"`, sonst testet man die alte Fassung.

## Als Nächstes

1. **`.env.example` fehlt weiterhin, und das ist jetzt dringend.** Im
   Repo-Wurzelverzeichnis liegt eine `.env` (52 Bytes), die von Git
   ausgeschlossen ist und deren Variablen nirgends dokumentiert sind. Ohne sie
   baut die App auf einem frischen Rechner nicht. Louis wollte den lokalen
   Ordner löschen, ich habe abgeraten, bis das gesichert ist.
   Zu tun: Louis um Erlaubnis fragen, die `.env` zu lesen, daraus eine
   `.env.example` mit Platzhaltern bauen und committen. Erst danach ist der
   Ordner gefahrlos löschbar (Repo: `lou1s19/shelf`, privat).

2. **Vorschau beim Text-Kürzel.** „Area screenshot to text" kopiert den erkannten
   Text stumm in die Zwischenablage. Es soll kurz ein Fenster zeigen, was kopiert
   wurde, und von selbst wieder verschwinden.
   Betroffen: `apps/desktop/src-tauri/src/recording.rs`
   (`recognize_screenshot_text_to_clipboard`) und ein kleines Fenster analog zum
   Aufnahme-Overlay in `apps/desktop/src-tauri/src/windows.rs`.
   Offen: Wie lange stehen bleiben, wo erscheinen (bei der Maus oder am Rand),
   und was passiert bei langem Text (kürzen oder scrollen).

3. **Tray-Symbol gestalterisch nochmal ansehen.** Das Cap-Symbol ist raus, das
   neue Regal-Motiv liest sich bei Menüleistengröße aber eher wie ein
   Gleichheitszeichen, und der Punkt der Instant-Variante wirkt wie ein Fleck.
   Technisch in Ordnung (Template-Modus, vier Zustände, Quellen unter
   `apps/desktop/src-tauri/icons/src/`), nur nicht unverwechselbar.
   Offen: erst live in der Menüleiste ansehen, dann entscheiden, ob eine andere
   Richtung her muss. Die alten Cap-Symbole stecken im Git-Verlauf, falls du
   vergleichen willst (Commit davor: `1c4809e89`).

4. **Kleinkram aus dem Geschwindigkeits-Umbau.** Alles gemessen, alles gering:
   - `apps/desktop/src/routes/target-select-overlay.tsx:1264`: zusätzliche 50 ms
     `setTimeout` nur im Bereichs-Weg. Rust wartet danach ohnehin, kann weg.
   - `crates/recording/src/screenshot.rs:63-105`: skalare BGRA-nach-RGB-Schleife,
     rund 50 ms für 3,7 Megapixel. Als Chunk-Iterator oder mit `rayon` deutlich
     schneller.
   - „Camera Only" als Aufnahmeziel fehlt im Tray-Menü, im Hauptfenster gibt es
     das. Die Feinauswahl welcher Bildschirm oder welches Fenster fehlt bewusst,
     dafür müsste bei jedem Öffnen die Fensterliste aufgezählt werden.

## Danach

5. **Eigener Update-Weg.** `apps/desktop/src-tauri/src/updates.rs` ist ein
   Platzhalter, der immer „keine Aktualisierung" meldet. Caps Endpunkt ist raus,
   weil er Shelf durch Cap ersetzt hätte. Nötig wäre ein eigener Release-Feed
   plus Signierschlüssel, oder die Funktion ganz aus den Einstellungen nehmen.

6. **Namen im Paketinneren.** Die mitgelieferten Hilfsprogramme heißen weiter
   `cap-cli`, `cap-exporter`, `cap-muxer`, die Projektendung ist `.cap` und das
   Deep-Link-Schema `cap-desktop`. Im Betrieb sichtbar ist davon nichts. Das
   Umbenennen der Endung berührt viele Stellen und braucht eine Migration für
   bestehende Aufnahmen.

7. **Sprachmodelle für die Untertitel** kommen von
   `github.com/CapSoftware/transcription-models`. Funktioniert, hängt aber an
   einem fremden Repo. Eigene Ablage oder ein anderer Anbieter wäre sauberer.

8. **Mehrsprachigkeit fehlt.** Alle Texte stehen fest im Code auf Englisch.

9. **CI deckt den Rust-Build nicht ab.** `.github/workflows/ci.yml` prüft nur
   Typecheck, Biome und `cargo fmt` auf Linux. Der eigentliche Build bräuchte
   macOS-Runner zum zehnfachen Minutenpreis und würde das Kontingent eines
   privaten Repos in wenigen Läufen aufbrauchen. Sobald das Repo öffentlich ist,
   sind macOS-Runner kostenlos, dann `cargo check -p cap-desktop` ergänzen.

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
