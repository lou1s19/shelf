# Oberflächen-Kriterien

Die Liste für den Skill `mac-app-selbsttest`. Jede Zeile ist an einem Screenshot mit
Ja oder Nein zu beantworten. Wer eine Zeile nicht am Bild entscheiden kann, schreibt
das hin, statt zu raten.

Screenshots ohne Fokusklau:

```sh
CLI=/Applications/Shelf.app/Contents/MacOS/cap-cli
"$CLI" targets --json                                   # Fenster-ID zum Titel
"$CLI" screenshot --window <id> --path /tmp/pruef/x.png
```

Seite ansteuern, ohne zu klicken:

```sh
open "shelf://action?value=$(python3 -c 'import json,urllib.parse;print(urllib.parse.quote(json.dumps({"open_settings":{"page":"hotkeys"}})))')"
```

## Überall

- Keine Reste aus dem Cap-Fork in sichtbarem Text: kein `cap`, kein „Cap", keine
  Cap-Adressen. Ausnahme sind Dateipfade zu den mitgelieferten Programmen
  (`cap-cli`, `cap-exporter`, `cap-muxer`), die heißen intern noch so.
- Kein abgeschnittener Text, keine überlappenden Elemente, keine leeren Karten.
- Dunkler Hintergrund, Text darauf lesbar.
- Keine Platzhalter wie „TODO", „Lorem" oder leere Klammern.

## Onboarding (erster Start)

Vorher den Zustand beiseitelegen, sonst wird es übersprungen:
`mv ~/Library/Application\ Support/de.shelf.desktop/store ~/.Trash/...`
Aufnahmen und Screenshots dabei **nicht** anfassen.

- Das Onboarding erscheint überhaupt.
- Keine Wolken und keine sonstige Deko ohne Aussage im Hintergrund.
- Die Schritte sind ohne Vorwissen verständlich.
- Die Berechtigungsschritte nennen, wofür die Berechtigung gebraucht wird.

## Einstellungen › Experimental

- Der Abschnitt „Command Line" ist da, ein eigener Menüpunkt „CLI" ist es **nicht**.
- Der Text nennt den Befehl `shelf`, nicht `cap`.
- Die Zeile „Command" zeigt einen Pfad, der auf `shelf` endet.

## Einstellungen › License

- Ohne Schlüssel steht dort, dass Shelf frei ist, nicht „kein Schlüssel gefunden"
  als Fehler.
- Die Versionsnummer stimmt mit der gebauten Fassung überein.
- Wenn nichts kostenpflichtig ist, taucht keine Liste gesperrter Funktionen auf.

## Einstellungen, alle Seiten

- Der Deeplink führt wirklich auf die angefragte Seite, auch wenn das Fenster
  schon offen war. (War bis 0.5.10 kaputt, siehe CHANGELOG.)
- Die linke Navigation hebt die Seite hervor, auf der man ist.
- Unten stehen Version und „Check for updates".
