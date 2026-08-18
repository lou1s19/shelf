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

echo "==> building ($MODE)"
cd "$REPO"
pnpm --dir apps/desktop tauri build "${BUILD_FLAGS[@]}" \
	--config src-tauri/tauri.prod.conf.json \
	--no-bundle=false 2>&1 | tail -5 || true

SOURCE="$BUNDLE_DIR/$APP_NAME"
if [ ! -d "$SOURCE" ]; then
	echo "no bundle at $SOURCE" >&2
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
