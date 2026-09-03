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
trap 'echo "==> stopped at line $LINENO" >&2' ERR

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

# A pid file holds one pid per line: the web server is started through pnpm, which keeps
# the actual server as a child, and killing only one of the two leaves the port taken.
running() {
	local pid_file="$1"
	[ -f "$pid_file" ] || return 1
	while read -r pid; do
		[ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && return 0
	done <"$pid_file"
	return 1
}

stop_pid_file() {
	local pid_file="$1"
	if [ -f "$pid_file" ]; then
		while read -r pid; do
			[ -n "$pid" ] && kill "$pid" 2>/dev/null || true
		done <"$pid_file"
		sleep 1
	fi
	rm -f "$pid_file"
}

# `|| true` because an empty port makes lsof exit 1, which under `pipefail` would end
# the script mid-assignment without a word.
port_owner() {
	lsof -ti "tcp:$WEB_PORT" 2>/dev/null | head -1 || true
}

start_web() {
	local owner
	owner="$(port_owner)"
	if [ -n "$owner" ]; then
		# Started earlier, possibly outside this script. Take it over, or `stop` and
		# `status` would talk about a server they do not know.
		if ! running "$WEB_PID_FILE"; then echo "$owner" >"$WEB_PID_FILE"; fi
		echo "==> web server already up on port $WEB_PORT"
		return
	fi

	echo "==> starting the web server on port $WEB_PORT"
	(cd "$REPO" && pnpm --dir apps/desktop localdev >"$WEB_LOG" 2>&1 &
		echo $! >"$WEB_PID_FILE")
	# Tauri loads the page on launch, so the app must not start before it answers.
	for _ in $(seq 1 60); do
		if curl -sf "http://localhost:$WEB_PORT" >/dev/null 2>&1; then
			owner="$(port_owner)"
			if [ -n "$owner" ] && ! grep -qx "$owner" "$WEB_PID_FILE"; then
				echo "$owner" >>"$WEB_PID_FILE"
			fi
			return
		fi
		sleep 1
	done
	echo "==> the web server did not answer, see $WEB_LOG" >&2
	exit 1
}

# A fresh clone has none of this, and each piece fails deep inside a later step:
# without node_modules the web server has no vinxi, without the native deps the
# ffmpeg crates look for a system ffmpeg that is not there, and without the
# sidecars the Tauri build script stops on a missing resource.
prepare() {
	# Cheap and lockfile driven, so it runs every time and picks up a changed lockfile.
	# The other two are minutes of download and compile and are checked by existence only:
	# they change with the ffmpeg version or the sidecar sources, which is rare enough to
	# delete `target/native-deps` or `src-tauri/binaries` by hand for.
	echo "==> checking node packages"
	(cd "$REPO" && pnpm install)

	if [ ! -f "$REPO/.cargo/config.toml" ] || [ ! -d "$REPO/target/native-deps" ]; then
		echo "==> fetching the native dependencies (ffmpeg, onnxruntime)"
		(cd "$REPO" && node scripts/setup.js)
	fi

	local triple
	triple="$(rustc -vV | sed -n 's|host: ||p')"
	for sidecar in cap-muxer cap-cli cap-exporter; do
		if [ ! -f "$REPO/apps/desktop/src-tauri/binaries/$sidecar-$triple" ]; then
			echo "==> building the sidecars (cap-muxer, cap-cli, cap-exporter), this takes a while"
			bash "$REPO/scripts/build-desktop-binaries.sh"
			break
		fi
	done
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
		prepare
		start_web
		build
		start_app
		;;
	reload)
		prepare
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
		running "$APP_PID_FILE" && echo "app:        running ($(tr '\n' ' ' <"$APP_PID_FILE"))" || echo "app:        stopped"
		running "$WEB_PID_FILE" && echo "web server: running ($(tr '\n' ' ' <"$WEB_PID_FILE"))" || echo "web server: stopped"
		echo "app log:    $APP_LOG"
		echo "web log:    $WEB_LOG"
		;;
	*)
		echo "usage: $0 [start|reload|stop|status]" >&2
		exit 2
		;;
esac
