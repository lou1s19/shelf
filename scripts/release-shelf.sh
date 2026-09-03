#!/usr/bin/env bash
# Builds a Shelf release for other people's Macs: signed, notarised, stapled,
# plus the update feed for the website.
#
#   scripts/release-shelf.sh 1.0.0
#
# Everything lands in target/release-out/. Nothing is uploaded from here.
#
# What this needs once, before the first run:
#   1. A Developer ID certificate in the login keychain (already there).
#   2. A stored notarisation profile, made with an app-specific password from
#      appleid.apple.com:
#        xcrun notarytool store-credentials shelf-notary \
#          --apple-id <apple-id> --team-id H8XJ9NV6ZQ --password <app-specific-password>
#   3. The updater signing key at ~/.shelf-licensing/tauri-update.key, whose
#      public half is already in tauri.prod.conf.json.
#
# Without notarisation the app opens on this Mac and nowhere else: on any other
# machine macOS reports it as damaged. That is why the check below refuses to
# build rather than producing a download that looks fine and is not.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"
IDENTITY="${SHELF_SIGN_IDENTITY:-Developer ID Application: Louis Saks (H8XJ9NV6ZQ)}"
NOTARY_PROFILE="${SHELF_NOTARY_PROFILE:-shelf-notary}"
UPDATE_KEY="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.shelf-licensing/tauri-update.key}"
BASE_URL="${SHELF_BASE_URL:-https://shelf-website-mu.vercel.app}"
APP_NAME="Shelf.app"
BUNDLE_DIR="$REPO/target/release/bundle/macos"
OUT="$REPO/target/release-out"

export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

if [ -z "$VERSION" ]; then
	echo "usage: $0 <version>   e.g. $0 1.0.0" >&2
	exit 2
fi

CARGO_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/apps/desktop/src-tauri/Cargo.toml" | head -1)"
if [ "$VERSION" != "$CARGO_VERSION" ]; then
	echo "version mismatch: you asked for $VERSION, Cargo.toml says $CARGO_VERSION" >&2
	echo "Bump apps/desktop/src-tauri/Cargo.toml first, so the feed and the app agree." >&2
	exit 1
fi

echo "==> checks"
security find-identity -v -p codesigning | grep -q "$IDENTITY" || {
	echo "no signing identity \"$IDENTITY\" in the keychain" >&2; exit 1; }
xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1 || {
	echo "no notarisation profile \"$NOTARY_PROFILE\"." >&2
	echo "Create it once (see the header of this script), then run again." >&2
	exit 1; }
[ -f "$UPDATE_KEY" ] || { echo "no updater key at $UPDATE_KEY" >&2; exit 1; }

TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
for SIDECAR in cap-muxer cap-cli cap-exporter; do
	if [ ! -f "$REPO/apps/desktop/src-tauri/binaries/$SIDECAR-$TRIPLE" ]; then
		echo "==> building sidecars"
		bash "$REPO/scripts/build-desktop-binaries.sh"
		break
	fi
done

echo "==> building $VERSION (release, 20 to 40 minutes)"
cd "$REPO"
BUILD_LOG="$(mktemp -t shelf-release)"
if ! pnpm --dir apps/desktop tauri build --config src-tauri/tauri.prod.conf.json \
	>"$BUILD_LOG" 2>&1; then
	echo "==> build failed" >&2
	tail -40 "$BUILD_LOG" >&2
	exit 1
fi
rm -f "$BUILD_LOG"

SOURCE="$BUNDLE_DIR/$APP_NAME"
[ -d "$SOURCE" ] || { echo "no bundle at $SOURCE" >&2; exit 1; }

[ -d "$OUT" ] && /bin/rm -rf "$OUT"
mkdir -p "$OUT"
# ditto drops the extended attributes an iCloud-synced folder keeps adding,
# which codesign refuses to sign around.
ditto --norsrc --noextattr --noacl "$SOURCE" "$OUT/$APP_NAME"
xattr -cr "$OUT/$APP_NAME"

echo "==> signing"
# Inside out: libraries, then frameworks, then executables, then the bundle.
# A bundle signed before its contents fails notarisation.
find "$OUT/$APP_NAME" -name "*.dylib" -type f -print0 |
	xargs -0 -n1 codesign --force --options runtime --timestamp -s "$IDENTITY" >/dev/null
for fw in "$OUT/$APP_NAME"/Contents/Frameworks/*.framework; do
	[ -d "$fw" ] || continue
	[ -d "$fw/Versions/A" ] && codesign --force --options runtime --timestamp -s "$IDENTITY" "$fw/Versions/A" >/dev/null
	codesign --force --options runtime --timestamp -s "$IDENTITY" "$fw" >/dev/null
done
for bin in "$OUT/$APP_NAME"/Contents/MacOS/*; do
	codesign --force --options runtime --timestamp -s "$IDENTITY" "$bin" >/dev/null
done
codesign --force --options runtime --timestamp \
	--entitlements "$REPO/apps/desktop/src-tauri/Entitlements.plist" \
	-s "$IDENTITY" "$OUT/$APP_NAME" >/dev/null
codesign --verify --strict --deep "$OUT/$APP_NAME"

echo "==> notarising the app (a few minutes)"
ditto -c -k --keepParent "$OUT/$APP_NAME" "$OUT/notarize-app.zip"
xcrun notarytool submit "$OUT/notarize-app.zip" \
	--keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$OUT/$APP_NAME"
rm -f "$OUT/notarize-app.zip"

echo "==> packing the download"
DMG="$OUT/Shelf-$VERSION.dmg"
STAGE="$OUT/dmg-stage"
mkdir -p "$STAGE"
ditto "$OUT/$APP_NAME" "$STAGE/$APP_NAME"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Shelf" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
/bin/rm -rf "$STAGE"
codesign --force --timestamp -s "$IDENTITY" "$DMG" >/dev/null

echo "==> notarising the download"
xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$DMG"

echo "==> packing the update"
# Built from the stapled app on purpose: the updater replaces the installed
# bundle with exactly this, so it has to carry the notarisation ticket too.
FEED_BUNDLE="$OUT/feed-bundle"
mkdir -p "$FEED_BUNDLE"
tar -czf "$FEED_BUNDLE/Shelf.app.tar.gz" -C "$OUT" "$APP_NAME"
pnpm --dir apps/desktop exec tauri signer sign \
	-f "$UPDATE_KEY" -p "" "$FEED_BUNDLE/Shelf.app.tar.gz" >/dev/null
node "$REPO/scripts/make-update-feed.mjs" \
	--version "$VERSION" --base-url "$BASE_URL" \
	--bundle-dir "$FEED_BUNDLE" --out "$OUT/website"

echo
echo "==> done"
echo "  download  : $DMG"
echo "  website   : $OUT/website/updates/"
echo
echo "Upload the DMG where the site's download button points, and the contents"
echo "of $OUT/website/ to the site root, keeping the paths."
echo "Then check that $BASE_URL/updates/latest.json answers with JSON."
