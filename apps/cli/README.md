# shelf CLI

Shelf's screen recording and screenshots, driven from the command line. The binary is built for
automation and for AI coding agents: every command speaks JSON, errors are machine-readable, and
recordings have an explicit start/stop lifecycle.

Everything runs locally. There is no account, no upload and no server.

## Install

Shelf Desktop → Settings → Command Line → Install CLI. That links the bundled binary into
`~/.shelf/bin` as `shelf`, so the app and the CLI are always the same build.

## The output convention (read this first)

- Pass `--json` (a global flag) to **any** command for machine-readable JSON on **stdout**. A
  command's own `--format json` works too; `--json` is the order-insensitive shortcut
  (`shelf --json targets` and `shelf targets --json` both work).
- **stdout** is the authoritative result. **stderr** is human-readable logs plus a final
  `error: <message>` line on failure.
- Failures exit **non-zero**. In `--json` mode a final object carries an `error` string field, so a
  single `"error" in obj` check detects failure across every command. Usage errors exit `2`.
- `record` and `export` stream **newline-delimited JSON (NDJSON)** events on stdout.
- Fetch the full machine-readable contract any time with **`shelf guide --json`**.

## Environment variables

| Variable             | Used by               | Notes                                     |
| -------------------- | --------------------- | ----------------------------------------- |
| `CAP_NO_MODIFY_PATH` | `desktop install-cli` | Set to skip editing shell profiles / PATH. |

## Typical agent workflow

```sh
shelf doctor --json                                # permissions and capture readiness
shelf targets --json                               # screens/windows/cameras/mics, ids feed the next steps
shelf record start --screen <id> --json --detach   # -> {"type":"started","recordingId","pid","path"}
# ... whatever needs to be captured happens here ...
shelf record stop --id <recordingId> --json        # -> {"type":"stopped","path", …}
shelf project validate <path.cap> --json           # confirm the recording is complete
shelf export <path.cap> --output out.mp4 --json
```

## Commands

- `shelf record start` / `record stop` / `record status` — record in the foreground, or `--detach`
  for the background.
- `shelf screenshot` — capture a still of a screen or window (`--json` → `{path,width,height}`).
- `shelf export` — render a `.cap` project to mp4/gif/mov. `--format` selects the container.
- `shelf export-preview` — render a single preview frame of an export.
- `shelf project inspect` / `validate` / `config get|set` — inspect and edit `.cap` projects.
- `shelf recordings list` — list the recordings in the desktop library, or in `--dir`.
- `shelf targets` (`screens`/`windows`/`cameras`/`mics`) — enumerate capture inputs.
- `shelf automations list` — list the automation rules configured in Shelf Desktop.
- `shelf doctor` / `selftest` / `version` / `guide` — diagnostics and the agent capability manifest.
- `shelf desktop status|install-cli|uninstall-cli` — manage the shim on PATH.
- `shelf completions <shell>` — completion scripts (bash/zsh/fish/powershell).

## Automations

Automations are `trigger → (conditions) → actions` rules authored in Shelf Desktop (Settings →
Automations) and stored in its store. The CLI shares that store and the same engine, so it runs the
same rules after `shelf screenshot` and after a `shelf record` finishes — for example "on screenshot,
save a copy to `~/Shots` and POST a webhook". Clipboard, OCR, notification and open-editor actions
are desktop-only and are skipped on the CLI; save, export, run command, webhook, reveal, apply preset
and delete all run. Inspect the active rules with `shelf automations list --json`.

Run `shelf --help` or `shelf <command> --help` for the full flag documentation.
