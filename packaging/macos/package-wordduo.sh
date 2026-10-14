#!/usr/bin/env bash
# WordDuo (this fork) for macOS: a universal WordDuo.app, signed with a Developer ID, notarized,
# stapled, on a drag-to-Applications DMG:
#
#   $DIST/WordDuo-<version>-macos-universal.dmg
#
# Usage: packaging/macos/package-wordduo.sh [--skip-build]
#
# Env:
#   WORDDUO_VERSION       version shown in Finder (default: 0.1.0)
#   MACOS_SIGN_IDENTITY   codesign identity (default: the keychain's "Developer ID Application")
#   NOTARY_PROFILE        notarytool keychain profile (default: shooft); "" skips notarization
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/macos"

SKIP_BUILD=0
[ "${1:-}" = "--skip-build" ] && SKIP_BUILD=1

export MACOSX_DEPLOYMENT_TARGET=11.0
APP_VERSION="${WORDDUO_VERSION:-0.1.0}"
IDENTITY="${MACOS_SIGN_IDENTITY:-$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)}"
PROFILE="${NOTARY_PROFILE-shooft}"
[ -n "$IDENTITY" ] || { echo "error: no Developer ID Application identity in the keychain" >&2; exit 1; }
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
WORK="$CARGO_TARGET_DIR/wordduo-package"
APP="$WORK/WordDuo.app"
DMG="$DIST/WordDuo-$APP_VERSION-macos-universal.dmg"

echo "==> WordDuo $APP_VERSION (WordCraft $VERSION), identity: $IDENTITY, notary profile: ${PROFILE:-none}"

if [ "$SKIP_BUILD" = 0 ]; then
  args=()
  for t in "${TARGETS[@]}"; do args+=(--target "$t"); done
  (cd "$ROOT" && cargo build --release --locked -p wordcraft "${args[@]}")
fi

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
inputs=()
for t in "${TARGETS[@]}"; do inputs+=("$CARGO_TARGET_DIR/$t/release/wordcraft"); done
lipo -create -output "$APP/Contents/MacOS/WordDuo" "${inputs[@]}"
lipo -info "$APP/Contents/MacOS/WordDuo"
cp "$ROOT/assets/app-icon/wordcraft.icns" "$APP/Contents/Resources/WordDuo.icns"
copy_font_licences "$APP/Contents/Resources"
# The WordCraft template, renamed: name, executable, icon, identifier, copyright.
sed -e "s/@VERSION@/$APP_VERSION/g" -e "s/@SHORT_VERSION@/$APP_VERSION/g" \
  -e "s/@BUILD_SHA@/${WORDCRAFT_BUILD_SHA:-unknown}/g" \
  -e "s#<string>WordCraft</string>#<string>WordDuo</string>#g" \
  -e "s#<string>ai.storyteller.wordcraft</string>#<string>io.github.jinsu35.wordduo</string>#" \
  -e "s#Copyright © the WordCraft authors.#Based on WordCraft. Copyright © the WordCraft authors and jinsu kim.#" \
  "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"

sign() { codesign --force --sign "$IDENTITY" --timestamp "$@"; }
notarize() {
  [ -n "$PROFILE" ] || return 0
  echo "==> notarizing $(basename "$1") (a few minutes)"
  xcrun notarytool submit "$1" --keychain-profile "$PROFILE" --wait --timeout 1h
}

sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/WordDuo"
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"
if [ -n "$PROFILE" ]; then
  ditto -c -k --keepParent "$APP" "$WORK/WordDuo-notarize.zip"
  notarize "$WORK/WordDuo-notarize.zip"
  xcrun stapler staple "$APP"
  spctl --assess --type execute -vvv "$APP"
fi

# A plain DMG (WordCraft's Finder background carries its own artwork).
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/WordDuo.app"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG" "$WORK/raw.dmg"
hdiutil makehybrid -hfs -hfs-volume-name "WordDuo" -hfs-openfolder "$STAGE" -o "$WORK/raw.dmg" "$STAGE"
hdiutil convert "$WORK/raw.dmg" -format UDZO -imagekey zlib-level=9 -o "$DMG"
rm -f "$WORK/raw.dmg"
sign "$DMG"
notarize "$DMG"
if [ -n "$PROFILE" ]; then
  xcrun stapler staple "$DMG"
  spctl --assess --type open --context context:primary-signature -vvv "$DMG"
fi
echo "==> done"
ls -lh "$DMG"
