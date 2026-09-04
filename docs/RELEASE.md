# Shelf veröffentlichen und später kostenpflichtig machen

Diese Datei beschreibt zwei Dinge: wie eine Version auf die Website kommt, und
wie Shelf später kostenpflichtig wird, ohne dass jemand auf der alten, freien
Fassung sitzen bleibt.

## Die zwei Schlüssel

Beide liegen in `~/.shelf-licensing/`, keiner davon im Repo.

| Datei | Wofür | Wenn er weg ist |
|---|---|---|
| `tauri-update.key` | signiert die Update-Pakete | Updates lassen sich nicht mehr ausliefern, jede installierte App braucht eine Neuinstallation von Hand |
| `secret.key` | signiert Lizenzschlüssel und die Policy-Datei | alle verkauften Schlüssel werden ungültig, es braucht eine neue App-Version mit neuem öffentlichen Schlüssel |

**Beide sind gesichert**, seit 2026-09-03: als AES-256-verschlüsseltes Abbild
unter `~/serv/Shelf-Builds/shelf-signing-keys.dmg` auf dem Homeserver. Das
Kennwort steht im Schlüsselbund unter „Shelf Signing Keys Backup" und gehört
zusätzlich in den Passwortmanager, sonst nützt die Sicherung nach einem
Plattenschaden nichts. Wie man sie zurückspielt, steht im Abbild in
`LIESMICH.txt`.

Werden die Schlüssel je erneuert, muss diese Sicherung im selben Zug
mitgemacht werden. Ohne sie ist die veröffentlichte App eine Sackgasse.

Die öffentlichen Hälften stecken schon im Code:
`tauri.prod.conf.json` (`pubkey`) und `apps/desktop/src-tauri/src/licensing.rs`
(`PUBLIC_KEY`).

## Eine Version veröffentlichen

1. Version in `apps/desktop/src-tauri/Cargo.toml` hochzählen.
2. `scripts/release-shelf.sh <version>` laufen lassen. Das Skript baut, signiert,
   lässt bei Apple notarisieren, klebt das Ticket an und legt alles in
   `target/release-out/` ab.
3. Hochladen:
   - die `.dmg` dorthin, wo der Download-Knopf der Website hinzeigt,
   - den Inhalt von `target/release-out/website/` in die Wurzel der Website,
     also `updates/latest.json` und `updates/<version>/Shelf.app.tar.gz`.
4. Nach dem Hochladen gegen die echten Adressen prüfen:

   ```sh
   node scripts/verify-update-feed.mjs --feed https://<domain>/updates/latest.json
   ```

   Das lädt das Paket von dort, wo die App es später holt, und prüft die
   Signatur gegen den Schlüssel im Build. Der Release-Lauf macht dasselbe schon
   lokal; dieser zweite Durchgang fängt Upload-Fehler und Fehlerseiten ab. Eine
   HTML-Fehlerseite statt JSON bemerkt sonst niemand, bis Updates monatelang
   stillstehen.
5. Git-Tag `v<version>` setzen und `CHANGELOG.md` ergänzen.

**Vor dem ersten Lauf einmalig nötig:** ein Notarisierungs-Profil im
Schlüsselbund. Dafür braucht es ein app-spezifisches Passwort von
appleid.apple.com:

```sh
xcrun notarytool store-credentials shelf-notary \
  --apple-id <apple-id> --team-id H8XJ9NV6ZQ --password <app-spezifisches-passwort>
```

Ohne Notarisierung startet die App auf keinem fremden Mac. macOS meldet dort
„beschädigt", nicht „nicht signiert", was die Ursache gut versteckt.

## Die Policy-Datei

Die App fragt alle sechs Stunden eine einzige Datei ab:
`https://<domain>/policy.txt`. Darin steht signiert, welche Version mindestens
nötig ist und welche Funktionen etwas kosten. Sie wird so erzeugt:

```sh
cargo run -p shelf-licensing --features mint --bin shelf-license -- policy \
  --minimum-version 0.0.0 \
  --download-url https://<domain>
```

Die Ausgabe ist eine Zeile. Die kommt als `policy.txt` auf die Website.

`--minimum-version 0.0.0` sperrt niemanden aus. Genau so sollte die erste
Fassung aussehen: die Prüfung läuft, ändert aber nichts.

Wichtige Eigenschaften:

- **Signiert.** Ohne gültige Signatur wird die Datei ignoriert. Ein fremder
  Server kann Shelf also weder sperren noch freischalten.
- **Kein Netz, kein Problem.** Schlägt der Abruf fehl, gilt die zuletzt
  gespeicherte Policy weiter. Eine schlechte Verbindung sperrt die App nie.
- **Nicht zurückdrehbar.** Eine ältere Policy überschreibt keine neuere. Ein
  alter Stand aus dem Cache kann eine gesetzte Untergrenze nicht wieder
  aufheben.

## Später kostenpflichtig machen

Der Ablauf hat drei Schritte und ist bewusst in dieser Reihenfolge.

**Schritt 1: die bezahlte Version bauen und hochladen.**
Version hochzählen (zum Beispiel auf `1.0.0`), veröffentlichen wie oben. Diese
Version enthält bereits die Schranken, sie sind nur noch nicht scharf.

**Schritt 2: Policy mit Untergrenze und Bezahlteil signieren.**

```sh
cargo run -p shelf-licensing --features mint --bin shelf-license -- policy \
  --minimum-version 1.0.0 \
  --grace-days 14 \
  --paid app \
  --download-url https://<domain> \
  --buy-url https://<domain>/kaufen \
  --message "Shelf 1.0 ist da. Ab hier braucht Shelf einen Lizenzschlüssel."
```

Hochladen als `policy.txt`. Ab dann passiert Folgendes:

- Jede installierte Version unter `1.0.0` zeigt vierzehn Tage lang einen
  Hinweis und danach ein Fenster, das zum Update zwingt. Ohne Update lässt sich
  nichts mehr aufnehmen, kein Screenshot machen, nichts exportieren.
- Wer aktualisiert hat, landet in `1.0.0` und braucht dort einen Schlüssel.

`--grace-days 0` weglassen heißt: sofort sperren. Vierzehn Tage sind
freundlicher und kosten nichts.

**Schritt 3: Schlüssel verkaufen.**

```sh
cargo run -p shelf-licensing --features mint --bin shelf-license -- license \
  --id <bestellnummer> --name "Vorname Nachname"
```

Die Ausgabe ist der Schlüssel, den der Käufer in Einstellungen › License
einfügt. Er wird lokal geprüft, es geht nichts an einen Server. `--valid-days`
setzt ein Ablaufdatum, ohne die Angabe läuft der Schlüssel nie ab.

### Nur Teile kostenpflichtig machen

Statt `--paid app` einzelne Funktionen angeben, mehrfach erlaubt:

```sh
--paid studio-recording --paid export
```

Welche Namen es gibt:

```sh
cargo run -p shelf-licensing --features mint --bin shelf-license -- features
```

Nennt die Policy eine Funktion, die eine ältere App-Version noch nicht kennt,
ignoriert diese Version sie. Genau dafür gibt es die Untergrenze: erst alle auf
eine Version holen, die die neuen Namen kennt, dann kassieren.

## Was das nicht kann

Shelf steht unter AGPLv3, geerbt vom Cap-Fork. Wer die App bekommt, hat Anspruch
auf den Quelltext, ebenfalls unter AGPLv3. Er darf die Bezahlschranke
herausnehmen und die Fassung ohne Schranke weitergeben. Geld verlangen ist
erlaubt, das Weitergeben verbieten nicht.

Die Schranke ist also ein Türschloss, kein Tresor. Sie sorgt dafür, dass Zahlen
der bequeme Weg ist. Wer sie umgehen will, kann es, und das lässt sich mit
keiner Technik ändern, solange die Lizenz AGPLv3 bleibt.

Praktisch heißt das auch: Bevor die erste Version an Fremde geht, muss der
Quellcode-Link auf der Website auf ein **öffentliches** Repo zeigen. Aktuell ist
`lou1s19/shelf` privat, und ein Link ins Private erfüllt die AGPL nicht.

## Während der Entwicklung

Die Prüfung lässt sich abschalten, ohne den Code anzufassen:

```sh
SHELF_POLICY_URL= pnpm dev:desktop
```

Leerer Wert heißt: kein Abruf, keine Untergrenze, alles frei. Ohne die Variable
verhält sich der Dev-Build wie die ausgelieferte App, was zum Testen der
Schranke der richtige Weg ist.
