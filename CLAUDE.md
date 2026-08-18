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
- **Im Repo liegt nur noch die Desktop-App.** Caps Website, Mobile-App,
  Chrome-Erweiterung, Discord-Bot, Media-Server und Cloud-Infrastruktur sind
  gelöscht. `git log` hat sie noch, falls du etwas nachschlagen willst.
- **Noch offen:** eigener Update-Weg, Deep-Link-Schema heißt weiter `cap-desktop`,
  Sidecar-Binaries heißen intern noch `cap-*`, Projektendung ist weiter `.cap`.

## Gepatchte Abhängigkeiten

Unter `vendor/` liegen Kopien von Fremd-Crates, die per `[patch.crates-io]` in der
Root-`Cargo.toml` eingebunden sind:

- `vendor/tao`, `vendor/wgpu-hal` — von Cap übernommen.
- `vendor/tauri-runtime-wry` — **eigener Patch.** Alle lesenden Zugriffe auf den
  Fensterspeicher laufen über `win_borrow()`. Hintergrund: Tauris Fensterspeicher
  ist ein `RefCell` hinter `unsafe impl Sync`; die Borrow-Sperre kann hängen
  bleiben, obwohl niemand schreibt, und dann killt jeder Fensterzugriff die App
  (tauri-apps/tauri#14801, #15003). Beim Öffnen des Aufnahme-Overlays passierte
  das zuverlässig. Details im `CHANGELOG.md`.
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
`~/Library/Application Support/so.cap.desktop.dev/store`, ihre Logs unter
`~/Library/Logs/so.cap.desktop/`. Panics landen zusätzlich in `panics.log`.

Die Screenshot-Tastenkürzel im Dev-Store: `Ctrl+Shift+1` ganzer Bildschirm,
`Ctrl+Shift+2` Bereich.
