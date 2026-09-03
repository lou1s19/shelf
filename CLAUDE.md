# CLAUDE.md

Read `AGENTS.md` for repository instructions.

## Dieser Fork

Das hier ist Louis' eigene App, entstanden aus einem Fork von
[CapSoftware/Cap](https://github.com/CapSoftware/Cap). Es gibt **keinen**
Pull-Request-Workflow mehr zurück nach oben. Änderungen landen hier.

Remotes:
- `origin` = `lou1s19/shelf` — das eigene Repo, aktuell privat. Hier wird gepusht.
- `cap-fork` = `lou1s19/Cap` — der alte Fork, bleibt als Archiv liegen.
- `upstream` = `CapSoftware/Cap` — nur zum Nachschlagen.

- **Lizenz:** AGPLv3 (siehe `LICENSE`), Teile MIT. Sobald die App an andere
  weitergegeben wird, muss der Quellcode mitgeliefert werden und die Lizenz
  bleibt AGPLv3. Das gilt auch für eine umbenannte eigene App.
- **Verlauf:** `CHANGELOG.md` im Root, neuester Eintrag oben.
- **Lokal, ohne Cloud:** Es gibt keine Konten, kein Hochladen, keine Teilen-Links,
  kein Cap Pro, keine Telemetrie und keinen Updater. Wer so etwas wieder einbaut,
  baut ein neues Produkt, nicht dieses.
- **Reine Menüleisten-App, seit 2026-08-19.** Es gibt **kein Hauptfenster**.
  Beim Start erscheint nichts, bedient wird über das Tray-Symbol. Fenster gibt es
  nur noch für Einstellungen, Editoren, Onboarding, Kamera, Teleprompter und die
  Overlays. Kein `CapWindowId::Main` wieder einführen: an dem Fenster hingen der
  Prewarm, das Beenden-Verhalten und die Geräteauswahl (siehe `CHANGELOG.md`).
  Geteilte Aufnahme-Bausteine liegen unter `apps/desktop/src/components/recording/`.
- **Im Repo liegt nur noch die Desktop-App und `packages/ui-solid`.** Caps
  Website, Mobile-App, Chrome-Erweiterung, Discord-Bot, Media-Server und seit
  2026-08-20 auch die Cloud-Pakete (Datenbank, Web-Backend, S3, SDKs) sind
  gelöscht. `git log` hat sie noch, falls du etwas nachschlagen willst.
- **Noch offen:** eigener Update-Weg, Sidecar-Binaries und Crates heißen intern
  noch `cap-*`, Projektendung ist weiter `.cap`. Das Deep-Link-Schema ist seit
  2026-08-20 `shelf://`, die Bundle-ID `de.shelf.desktop`.
- **Tray-Menü:** Die Symbole im Menü werden zur Laufzeit aus SF Symbols
  gezeichnet (`apps/desktop/src-tauri/src/tray_icons.rs`), nicht als PNG
  mitgeliefert. Grund steht im Modulkopf. Keine Geräteauswahl im Tray, die
  wohnt in Einstellungen › Geräte.

## Gepatchte Abhängigkeiten

Unter `vendor/` liegen Kopien von Fremd-Crates, die per `[patch.crates-io]` in der
Root-`Cargo.toml` eingebunden sind:

- `vendor/tao`, `vendor/wgpu-hal` — von Cap übernommen.
- `vendor/tauri-runtime-wry` — **eigener Patch.** Alle Zugriffe auf den
  Fensterspeicher laufen über `win_borrow()` (lesend) bzw. `win_borrow_mut()`,
  `win_insert_window()`, `win_remove_window()`, `win_detach_window()` (schreibend).
  Hintergrund: Tauris Fensterspeicher ist ein `RefCell` hinter `unsafe impl Sync`;
  die Borrow-Sperre kann hängen bleiben, obwohl niemand schreibt, und dann killt
  jeder Fensterzugriff die App (tauri-apps/tauri#14801, #15003). Beim Öffnen des
  Aufnahme-Overlays und beim Schließen eines Editor-Fensters passierte das
  zuverlässig. Nie wieder `cell.borrow_mut()` direkt aufrufen: Der Panic landet auf
  dem Main-Thread mitten in der Event-Schleife und hinterlässt eine App ohne
  Event-Schleife. Details im `CHANGELOG.md`.
  **Nicht wegoptimieren**, ohne vorher zu prüfen, ob eine neuere Tauri-Version
  das Problem behoben hat.

## Bauen und testen (macOS)

```sh
# Rust liegt bei Louis unter rustup von Homebrew, nicht im Standard-PATH:
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

pnpm dev:desktop                       # kompletter Dev-Lauf (Vite + Tauri)
pnpm --dir apps/desktop localdev       # nur der Vite-Server auf Port 3002
cargo build --no-default-features -p cap-desktop   # nur die Desktop-App
./target/debug/cap-desktop             # Binary direkt starten (Vite muss laufen)
```

Zum Ausprobieren ohne neue Installation gibt es `scripts/dev-shelf.sh`:

```sh
scripts/dev-shelf.sh start     # Webserver + Debug-App starten
scripts/dev-shelf.sh reload    # nach einer Rust-Aenderung neu bauen und neu starten
scripts/dev-shelf.sh stop
scripts/dev-shelf.sh status    # laeuft was, und wo liegen die Logs
```

Frontend-Aenderungen landen ohne `reload` in der laufenden App. Die Debug-App laeuft
neben der installierten Shelf.app, mit eigenem Store und eigener Bildschirmaufnahme-
Berechtigung.

Die Dev-App speichert ihren Zustand unter
`~/Library/Application Support/de.shelf.desktop.dev/store`, ihre Logs unter
`~/Library/Logs/de.shelf.desktop/`. Panics landen zusätzlich in `panics.log`.

Die Screenshot-Tastenkürzel im Dev-Store: `Ctrl+Shift+7` ganzer Bildschirm,
`Ctrl+Shift+8` Bereich. Bewusst andere als in der installierten Shelf
(`Ctrl+Shift+1` und `Ctrl+Shift+2`): laufen beide gleichzeitig, bekommt nur die
App das Kürzel, die es zuerst registriert hat, und das ist die installierte.
