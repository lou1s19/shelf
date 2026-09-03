#!/usr/bin/env bash
# Builds Shelf and installs it to /Applications, signed with the local Developer ID.
#
#   scripts/install-shelf.sh          fast build for testing (debug, minutes)
#   scripts/install-shelf.sh release  optimised build (release, much slower)
#
# Both variants carry the same bundle identifier and the same signing identity,
# so macOS keeps the permissions you already granted.
#
# Two macOS traps this works around:
#   1. The repo lives in an iCloud-synced folder. Files there constantly pick up
#      extended attributes that codesign refuses, so the bundle is copied out
#      with ditto (which drops them) before signing.
#   2. The bundled media libraries ship with a foreign team's signature. macOS
#      refuses to load them into a process signed by someone else, so every
#      dylib is re-signed from the inside out.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-debug}"
IDENTITY="${SHELF_SIGN_IDENTITY:-Developer ID Application: Louis Saks (H8XJ9NV6ZQ)}"
APP_NAME="Shelf.app"
TARGET="/Applications/$APP_NAME"

export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

case "$MODE" in
	debug)   BUILD_FLAGS=(--debug); BUNDLE_DIR="$REPO/target/debug/bundle/macos" ;;
	release) BUILD_FLAGS=();        BUNDLE_DIR="$REPO/target/release/bundle/macos" ;;
	*) echo "usage: $0 [debug|release]" >&2; exit 2 ;;
esac

# A fresh clone has no sidecars, and tauri only fails once it is deep into the
# Rust build. Build them up front instead.
TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
for SIDECAR in cap-muxer cap-cli cap-exporter; do
	if [ ! -f "$REPO/apps/desktop/src-tauri/binaries/$SIDECAR-$TRIPLE" ]; then
		echo "==> building sidecars (cap-muxer, cap-cli, cap-exporter)"
		bash "$REPO/scripts/build-desktop-binaries.sh"
		break
	fi
done

echo "==> building ($MODE)"
cd "$REPO"
# The build log goes to a file rather than the terminal, but a failure has to
# stop here. It used to end in `|| true` with only the last five lines shown, so
# a broken build installed the previous bundle and still reported success.
BUILD_LOG="$(mktemp -t shelf-build)"
# macOS ships bash 3.2, where an empty array under `set -u` counts as unbound.
# The release build passes no flags, so expand it only when it has entries.
# `--bundles app` skips the DMG, and the extra config switches off the updater archive:
# that one is signed with the release key, which a local test install has no business
# needing. Both only affect what is packaged, not the app itself.
if ! pnpm --dir apps/desktop tauri build ${BUILD_FLAGS[@]+"${BUILD_FLAGS[@]}"} \
	--bundles app --config src-tauri/tauri.prod.conf.json \
	--config '{"bundle":{"createUpdaterArtifacts":false}}' >"$BUILD_LOG" 2>&1; then
	echo "==> build failed, nothing installed" >&2
	tail -40 "$BUILD_LOG" >&2
	exit 1
fi
tail -5 "$BUILD_LOG"
rm -f "$BUILD_LOG"

SOURCE="$BUNDLE_DIR/$APP_NAME"
if [ ! -d "$SOURCE" ]; then
	echo "no bundle at $SOURCE" >&2
	exit 1
fi

# A bundle older than the newest source file means the build did not actually
# produce anything new, which is how a stale app got installed unnoticed.
NEWEST_SOURCE="$(find "$REPO/apps" "$REPO/crates" "$REPO/packages" -type f \
	\( -name "*.rs" -o -name "*.ts" -o -name "*.tsx" -o -name "*.json" \) \
	-not -path "*/node_modules/*" -not -path "*/.output/*" -not -path "*/dist/*" -newer "$SOURCE/Contents/MacOS/$(basename "$APP_NAME" .app)" \
	-print -quit 2>/dev/null || true)"
if [ -n "$NEWEST_SOURCE" ]; then
	echo "==> bundle is older than $NEWEST_SOURCE, refusing to install a stale build" >&2
	exit 1
fi

echo "==> installing to $TARGET"
if pgrep -f "$TARGET/Contents" >/dev/null 2>&1; then
	pkill -f "$TARGET/Contents" || true
	sleep 1
fi
[ -d "$TARGET" ] && /bin/rm -rf "$TARGET"
# ditto drops resource forks, extended attributes and ACLs on the way out
ditto --norsrc --noextattr --noacl "$SOURCE" "$TARGET"
xattr -cr "$TARGET"

echo "==> signing"
# inside out: libraries, then the framework, then the executables, then the bundle
find "$TARGET" -name "*.dylib" -type f -print0 |
	xargs -0 -n1 codesign --force --options runtime -s "$IDENTITY" >/dev/null 2>&1
for fw in "$TARGET"/Contents/Frameworks/*.framework; do
	[ -d "$fw" ] || continue
	[ -d "$fw/Versions/A" ] && codesign --force --options runtime -s "$IDENTITY" "$fw/Versions/A" >/dev/null 2>&1
	codesign --force --options runtime -s "$IDENTITY" "$fw" >/dev/null 2>&1
done
for bin in "$TARGET"/Contents/MacOS/*; do
	codesign --force --options runtime -s "$IDENTITY" "$bin" >/dev/null 2>&1
done
codesign --force --options runtime -s "$IDENTITY" "$TARGET" >/dev/null 2>&1

codesign --verify --strict --deep "$TARGET"
echo "==> $TARGET is signed and ready"
