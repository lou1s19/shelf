#!/usr/bin/env bash
# Runs Shelf from the debug build so a change can be tried out without building and
# installing a new Shelf.app, which takes minutes.
#
#   scripts/dev-shelf.sh start     start the web server and the app
#   scripts/dev-shelf.sh reload    rebuild the Rust side and restart the app
#   scripts/dev-shelf.sh stop      stop both
#   scripts/dev-shelf.sh status    what is running, and where the logs are
#
# Frontend files need no reload at all: the web server pushes them into the running
# app. Only Rust changes need `reload`, and only the changed crate is rebuilt.
#
# The dev app is a second app next to the installed one. It carries its own settings
# (`de.shelf.desktop.dev`), so macOS asks for screen recording permission once, and the
# shortcuts are whatever that store holds, not the ones of the installed Shelf.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${TMPDIR:-/tmp}/shelf-dev"
APP_LOG="$STATE_DIR/app.log"
WEB_LOG="$STATE_DIR/web.log"
APP_PID_FILE="$STATE_DIR/app.pid"
WEB_PID_FILE="$STATE_DIR/web.pid"
WEB_PORT=3002
BINARY="$REPO/target/debug/cap-desktop"

export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
mkdir -p "$STATE_DIR"

running() {
	local pid_file="$1"
	[ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null
}

stop_pid_file() {
	local pid_file="$1"
	if running "$pid_file"; then
		kill "$(cat "$pid_file")" 2>/dev/null || true
		# The web server spawns children that keep the port; give them a moment to go.
		sleep 1
	fi
	rm -f "$pid_file"
}

start_web() {
	if running "$WEB_PID_FILE" || lsof -ti "tcp:$WEB_PORT" >/dev/null 2>&1; then
		echo "==> web server already up on port $WEB_PORT"
		return
	fi
	echo "==> starting the web server on port $WEB_PORT"
	(cd "$REPO" && pnpm --dir apps/desktop localdev >"$WEB_LOG" 2>&1 &
		echo $! >"$WEB_PID_FILE")
	# Tauri loads the page on launch, so the app must not start before it answers.
	for _ in $(seq 1 60); do
		if curl -sf "http://localhost:$WEB_PORT" >/dev/null 2>&1; then
			return
		fi
		sleep 1
	done
	echo "==> the web server did not answer, see $WEB_LOG" >&2
	exit 1
}

build() {
	echo "==> building (only what changed)"
	(cd "$REPO" && cargo build --no-default-features -p cap-desktop)
}

start_app() {
	stop_pid_file "$APP_PID_FILE"
	echo "==> starting the app"
	("$BINARY" >"$APP_LOG" 2>&1 &
		echo $! >"$APP_PID_FILE")
	sleep 2
	if ! running "$APP_PID_FILE"; then
		echo "==> the app quit right away, see $APP_LOG" >&2
		tail -20 "$APP_LOG" >&2
		exit 1
	fi
	echo "==> up. Shelf sits in the menu bar, log: $APP_LOG"
}

case "${1:-start}" in
	start)
		start_web
		build
		start_app
		;;
	reload)
		start_web
		build
		start_app
		;;
	stop)
		stop_pid_file "$APP_PID_FILE"
		stop_pid_file "$WEB_PID_FILE"
		echo "==> stopped"
		;;
	status)
		running "$APP_PID_FILE" && echo "app:        running ($(cat "$APP_PID_FILE"))" || echo "app:        stopped"
		running "$WEB_PID_FILE" && echo "web server: running ($(cat "$WEB_PID_FILE"))" || echo "web server: stopped"
		echo "app log:    $APP_LOG"
		echo "web log:    $WEB_LOG"
		;;
	*)
		echo "usage: $0 [start|reload|stop|status]" >&2
		exit 2
		;;
esac
