# Was auf die Website gehört

Diese Dateien gehören in die Wurzel von `lou1s19/shelf-website`, damit die
ausgelieferte App sie unter den erwarteten Adressen findet.

```
policy.txt                              <- liegt hier, unverändert hochladen
updates/latest.json                     <- kommt aus scripts/release-shelf.sh
updates/<version>/Shelf.app.tar.gz      <- kommt aus scripts/release-shelf.sh
Shelf-<version>.dmg                     <- der Download, kommt aus dem gleichen Lauf
```

`policy.txt` ist der Schalter für später: Untergrenze und Bezahlteile. Die
aktuelle Fassung sperrt niemanden und macht nichts kostenpflichtig. Wie sie neu
erzeugt wird, steht in `docs/RELEASE.md`.

Beim Hosting auf Vercel zwei Dinge beachten:

- Die Dateien müssen als statische Dateien ausgeliefert werden, nicht durch eine
  Rewrite-Regel auf die Startseite laufen. Sonst antwortet der Server mit HTML,
  und Shelf hält das zu Recht für keine gültige Antwort.
- `latest.json` und `policy.txt` sollten nicht lange zwischengespeichert werden
  (`Cache-Control: max-age=300` reicht). Sonst dauert eine gesetzte Untergrenze
  Stunden, bis sie ankommt.

Die `.dmg` ist rund 200 MB und damit zu groß für ein Git-Repo. Sie gehört in
einen externen Speicher (zum Beispiel Cloudflare R2) und wird von der Website
nur verlinkt. Dieselbe Adresse muss dann in `policy.txt` als `--download-url`
stehen, damit das Update-Fenster den richtigen Ort nennt.
