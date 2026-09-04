#!/usr/bin/env bash
# Wraps a finished Shelf.app into the download image: app on the left, an
# Applications shortcut on the right, and a background that says to drag one
# onto the other.
#
#   scripts/make-dmg.sh <Shelf.app> <output.dmg>
#
# Why this is not Tauri's job: Tauri builds its image while bundling, from the
# app as it is before signing and notarisation, so that image cannot be shipped.
# Why it is not a plain `hdiutil create` either: the background is referenced
# from the volume's .DS_Store by an alias that is created on that volume, so it
# cannot be copied in from somewhere else. It has to be set on a mounted
# read-write image, which is what Finder is for here.
#
# The same approach as Orbly's scripts/make-dmg.sh, deliberately: it is proven,
# and both apps use the same background template.
#
# This opens a Finder window for a few seconds while it lays the window out.
# There is no way around that; setting a DMG background means talking to Finder.

set -euo pipefail

APP="${1:-}"
DMG="${2:-}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKGROUND="$REPO/apps/desktop/src-tauri/assets/dmg-background.png"
VOLUME="Shelf"

# The background is drawn for exactly this window, and the icon positions match
# the ones in tauri.conf.json. Change one number and the arrow points nowhere.
WIN_WIDTH=660
WIN_HEIGHT=379
APP_X=180
APP_Y=140
FOLDER_X=480
FOLDER_Y=140
ICON_SIZE=128

if [ -z "$APP" ] || [ -z "$DMG" ]; then
	echo "usage: $0 <Shelf.app> <output.dmg>" >&2
	exit 2
fi
[ -d "$APP" ] || { echo "no app bundle at $APP" >&2; exit 1; }
[ -f "$BACKGROUND" ] || { echo "no background image at $BACKGROUND" >&2; exit 1; }

WORK="$(mktemp -d -t shelf-dmg)"
trap '/bin/rm -rf "$WORK"' EXIT
STAGE="$WORK/stage"
RW_DMG="$WORK/rw.dmg"

mkdir -p "$STAGE/.background"
ditto "$APP" "$STAGE/$(basename "$APP")"
ln -s /Applications "$STAGE/Applications"
cp "$BACKGROUND" "$STAGE/.background/background.png"

# Read-write first: the window layout lives inside the volume and can only be
# written while it is mounted.
SIZE_KB=$(( $(du -sk "$STAGE" | cut -f1) + 20000 ))
hdiutil create -srcfolder "$STAGE" -volname "$VOLUME" -fs HFS+ \
	-format UDRW -size "${SIZE_KB}k" "$RW_DMG" >/dev/null

# Mount without a fixed mountpoint and read back the real name: if a volume
# called "Shelf" is already mounted, for example the released download, macOS
# names this one "Shelf 1", and scripting `disk "Shelf"` would then lay out
# somebody else's image instead of this one.
ATTACH="$(hdiutil attach "$RW_DMG" -nobrowse -noautoopen)"
MOUNT_DIR="$(printf '%s' "$ATTACH" | grep -o '/Volumes/.*' | tail -1)"
[ -d "$MOUNT_DIR" ] || { echo "could not mount $RW_DMG" >&2; exit 1; }
VOLUME_NAME="$(basename "$MOUNT_DIR")"
trap 'hdiutil detach "$MOUNT_DIR" -quiet 2>/dev/null || true; /bin/rm -rf "$WORK"' EXIT

echo "==> laying out the window (volume: $VOLUME_NAME)"
LAID_OUT=yes
osascript >/dev/null 2>&1 <<EOF || LAID_OUT=no
tell application "Finder"
  tell disk "$VOLUME_NAME"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {200, 120, $((200 + WIN_WIDTH)), $((120 + WIN_HEIGHT + 22))}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to $ICON_SIZE
    set background picture of opts to file ".background:background.png"
    set position of item "$(basename "$APP")" of container window to {$APP_X, $APP_Y}
    set position of item "Applications" of container window to {$FOLDER_X, $FOLDER_Y}
    close
    open
    update without registering applications
    delay 2
    close
  end tell
end tell
EOF
sync

if [ "$LAID_OUT" = "no" ]; then
	# Refuse rather than ship a download that opens as a bare file list. Usually
	# this means Terminal is not allowed to control Finder yet, under
	# System Settings > Privacy & Security > Automation.
	echo "Finder could not be scripted, so the window would stay empty." >&2
	echo "Allow the terminal to control Finder under" >&2
	echo "System Settings > Privacy & Security > Automation, then run again." >&2
	exit 1
fi

hdiutil detach "$MOUNT_DIR" -quiet
trap '/bin/rm -rf "$WORK"' EXIT

hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -o "$DMG" >/dev/null
echo "==> $DMG"
