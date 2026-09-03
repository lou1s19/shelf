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
- **Lokal, ohne Cloud:** Es gibt keine Konten, kein Hochladen, keine Teilen-Links
  und keine Telemetrie. Aufnahmen und Screenshots verlassen den Rechner nie.
  Seit 2026-09-03 gibt es genau zwei ausgehende Abrufe, beide ohne Nutzerdaten:
  der Update-Feed und `policy.txt` (Mindestversion und Bezahlteile, siehe unten).
  Wer darüber hinaus etwas hochlädt, baut ein neues Produkt, nicht dieses.
- **Update-Zwang und Bezahlschranke, seit 2026-09-03 eingebaut und inaktiv.**
  `crates/licensing` hält das signierte Format, `apps/desktop/src-tauri/src/licensing.rs`
  die Anbindung. Es gibt genau **eine** Durchsetzungsstelle, `licensing::require`,
  aufgerufen in `start_recording`, `take_screenshot` und den Export-Befehlen.
  Neue kostenpflichtige Funktionen dort anhängen, nicht eigene Prüfungen bauen.
  Zwei Regeln, die nicht verhandelbar sind: **kein Netz sperrt nie** (die zuletzt
  gespeicherte Policy gilt weiter), und **eine unlesbare Versionsangabe sperrt
  nie** (unser Fehler, nicht der des Nutzers). Ablauf und Schaltbefehle stehen in
  `docs/RELEASE.md`. Zum Entwickeln schaltet `SHELF_POLICY_URL=` alles ab.
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
- **Noch offen:** Sidecar-Binaries und Crates heißen intern noch `cap-*`,
  Projektendung ist weiter `.cap`. Das Deep-Link-Schema ist seit
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

Die Dev-App speichert ihren Zustand unter
`~/Library/Application Support/de.shelf.desktop.dev/store`, ihre Logs unter
`~/Library/Logs/de.shelf.desktop/`. Panics landen zusätzlich in `panics.log`.

Die Screenshot-Tastenkürzel im Dev-Store: `Ctrl+Shift+1` ganzer Bildschirm,
`Ctrl+Shift+2` Bereich.

## Veröffentlichen

`scripts/release-shelf.sh <version>` baut, signiert, notarisiert und erzeugt den
Update-Feed. Die beiden Signaturschlüssel liegen in `~/.shelf-licensing/` und
gehören gesichert: ohne sie lassen sich weder Updates ausliefern noch
Lizenzschlüssel ausstellen. Der ganze Ablauf steht in `docs/RELEASE.md`.
