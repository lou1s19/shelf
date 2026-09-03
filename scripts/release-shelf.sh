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
# Read first, match after: under `set -o pipefail` a `grep -q` that exits on the
# first match can SIGPIPE the left-hand side, and the pipeline then reports
# failure even though the identity was found.
IDENTITIES="$(security find-identity -v -p codesigning)"
case "$IDENTITIES" in
	*"$IDENTITY"*) ;;
	*) echo "no signing identity \"$IDENTITY\" in the keychain" >&2; exit 1 ;;
esac
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

# Everything up to the finished DMG happens outside the repo. The repo sits on
# the iCloud-synced Desktop, and iCloud keeps putting extended attributes back on
# files there. codesign refuses to sign a binary carrying them ("resource fork,
# Finder information, or similar detritus not allowed"), and clearing them first
# does not help, because the next sync puts them back mid-run.
WORK="$(mktemp -d -t shelf-release)"
trap '/bin/rm -rf "$WORK"' EXIT
APP="$WORK/$APP_NAME"

# ditto drops resource forks, extended attributes and ACLs on the way out.
ditto --norsrc --noextattr --noacl "$SOURCE" "$APP"
xattr -cr "$APP"

echo "==> signing"
# Inside out: libraries, then frameworks, then executables, then the bundle.
# A bundle signed before its contents fails notarisation.
find "$APP" -name "*.dylib" -type f -print0 |
	xargs -0 -n1 codesign --force --options runtime --timestamp -s "$IDENTITY" >/dev/null
for fw in "$APP"/Contents/Frameworks/*.framework; do
	[ -d "$fw" ] || continue
	if [ -d "$fw/Versions/A" ]; then
		codesign --force --options runtime --timestamp -s "$IDENTITY" "$fw/Versions/A" >/dev/null
	fi
	codesign --force --options runtime --timestamp -s "$IDENTITY" "$fw" >/dev/null
done
for bin in "$APP"/Contents/MacOS/*; do
	codesign --force --options runtime --timestamp -s "$IDENTITY" "$bin" >/dev/null
done
codesign --force --options runtime --timestamp \
	--entitlements "$REPO/apps/desktop/src-tauri/Entitlements.plist" \
	-s "$IDENTITY" "$APP" >/dev/null
codesign --verify --strict --deep "$APP"

echo "==> notarising the app (a few minutes)"
ditto -c -k --keepParent "$APP" "$WORK/notarize-app.zip"
xcrun notarytool submit "$WORK/notarize-app.zip" \
	--keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$APP"
rm -f "$WORK/notarize-app.zip"

echo "==> packing the download"
DMG="$WORK/Shelf-$VERSION.dmg"
STAGE="$WORK/dmg-stage"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/$APP_NAME"
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
FEED_BUNDLE="$WORK/feed-bundle"
mkdir -p "$FEED_BUNDLE"
tar -czf "$FEED_BUNDLE/Shelf.app.tar.gz" -C "$WORK" "$APP_NAME"
pnpm --dir apps/desktop exec tauri signer sign \
	-f "$UPDATE_KEY" -p "" "$FEED_BUNDLE/Shelf.app.tar.gz" >/dev/null
node "$REPO/scripts/make-update-feed.mjs" \
	--version "$VERSION" --base-url "$BASE_URL" \
	--bundle-dir "$FEED_BUNDLE" --out "$WORK/website"

# Only the finished, signed artefacts come back into the repo. Nothing here is
# signed again afterwards, so iCloud may decorate them all it likes.
# Written as an `if` rather than `[ -d ... ] && ...`: that form is one list, and
# when the test fails the list fails, which under `set -e` ends the script. On a
# first run the directory does not exist yet, so it aborted before building.
if [ -d "$OUT" ]; then
	/bin/rm -rf "$OUT"
fi
mkdir -p "$OUT"
ditto "$DMG" "$OUT/Shelf-$VERSION.dmg"
ditto "$WORK/website" "$OUT/website"
DMG="$OUT/Shelf-$VERSION.dmg"

echo "==> checking the update is installable"
# Proves the package can actually be installed by the app we just built: right
# key, and the signature matches these exact bytes. A feed that fails here
# fails silently at the user, months later, when the first update never lands.
node "$REPO/scripts/verify-update-feed.mjs" \
	--feed "$WORK/website/updates/latest.json" \
	--package "$FEED_BUNDLE/Shelf.app.tar.gz"

echo
echo "==> done"
echo "  download  : $DMG"
echo "  website   : $OUT/website/updates/"
echo
echo "Upload the DMG where the site's download button points, and the contents"
echo "of $OUT/website/ to the site root, keeping the paths."
echo "Then check that $BASE_URL/updates/latest.json answers with JSON."
