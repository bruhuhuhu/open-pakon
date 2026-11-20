#!/bin/bash
# Personal build of Pakon for an Intel Mac (x86_64), macOS 13 or later.
#
# The upstream packaging/macos/build.sh only runs on Apple silicon and
# lipo-joins a universal app. This script compiles the native Intel binary
# and assembles an ad-hoc signed Pakon.app. It is not notarized and it does
# not embed Sparkle, so it will not install an upstream release over itself.
#
#   packaging/macos/build-intel.sh
#
# Output: dist/macos/Pakon.app and dist/macos/Pakon.dmg
# See BUILD-PERSONAL.md for the prerequisites.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
VERSION="${1:-1.1.0-trim}"
CACHE="$HERE/.cache"
OUT="$REPO/dist/macos"
APP="$OUT/Pakon.app"
DEPLOY=13.0
TARGET="x86_64-apple-darwin"

step() { printf '\n==> %s\n' "$*"; }

[ "$(uname -s)" = "Darwin" ] || { echo "build-intel.sh runs on macOS (this machine is $(uname -s))" >&2; exit 1; }
[ "$(uname -m)" = "x86_64" ] || { echo "this script is for an Intel Mac (uname -m is $(uname -m)). On Apple silicon use packaging/macos/build.sh" >&2; exit 1; }
[[ "$VERSION" =~ ^[0-9]+(\.[0-9]+){1,2}(-[A-Za-z0-9.]+)?$ ]] \
    || { echo "version must look like 1.1.0 or 1.1.0-trim, got '$VERSION'" >&2; exit 1; }

export MACOSX_DEPLOYMENT_TARGET="$DEPLOY"
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"

command -v cargo >/dev/null || { echo "install Rust: https://rustup.rs then re-run" >&2; exit 1; }
rustup target list --installed | grep -qx "$TARGET" \
    || { echo "missing Rust target: rustup target add $TARGET" >&2; exit 1; }
command -v cmake >/dev/null || { echo "install CMake: brew install cmake" >&2; exit 1; }
command -v npm >/dev/null || { echo "install Node.js 22.12+ (or 20.19+): brew install node@22" >&2; exit 1; }

echo "Intel build $VERSION for macOS $DEPLOY"
echo "reports: off; updates: off (no Sparkle, version $VERSION)"

mkdir -p "$CACHE" "$OUT"
rm -rf "$APP" "$OUT/Pakon.dmg"

# --- icon (once) -------------------------------------------------------------
if [ ! -f "$CACHE/AppIcon.icns" ]; then
    step "icon"
    PY=""
    for p in python3 "$REPO/.venv/bin/python"; do
        if [ -n "$p" ] && "$p" -c 'import PIL' 2>/dev/null; then PY="$p"; break; fi
    done
    [ -n "$PY" ] || { echo "the icon needs Python 3 with Pillow: python3 -m pip install --user pillow" >&2; exit 1; }
    rm -rf "$CACHE/AppIcon.iconset"
    "$PY" -B "$HERE/make_icon.py" "$CACHE/AppIcon.iconset"
    iconutil -c icns "$CACHE/AppIcon.iconset" -o "$CACHE/AppIcon.icns"
fi

# --- the binary --------------------------------------------------------------
step "pakon_app (x86_64)"
(cd "$REPO/rust" && cargo build --release --locked -p pakon-app --target "$TARGET")
# Unicorn's JIT must be the Intel backend (packaging/macos/cc-x86_64).
CH="$(ls -t "$REPO"/rust/target/"$TARGET"/release/build/unicorn-engine-sys-*/out/build/config-host.h | head -1)"
grep -q "^#define HOST_X86_64 1" "$CH" || { echo "Unicorn built for the wrong host ($CH)." \
    "From rust/: cargo clean --release -p unicorn-engine-sys --target $TARGET" >&2; exit 1; }
BIN="$REPO/rust/target/$TARGET/release/pakon_app"
[ -x "$BIN" ] || { echo "no binary at $BIN" >&2; exit 1; }

"$REPO/packaging/build-web.sh"

# --- the bundle --------------------------------------------------------------
step "bundle Pakon.app"
RES="$APP/Contents/Resources"
mkdir -p "$APP/Contents/MacOS" "$RES/web" "$RES/resources/f135plus" "$RES/firmware"
cp "$BIN" "$APP/Contents/MacOS/Pakon"
cp -R "$REPO/web/static" "$RES/web/static"
cp "$REPO"/resources/*.pakscan "$REPO"/resources/*.pakfw "$RES/resources/"
cp "$REPO"/resources/f135plus/*.pakscan "$RES/resources/f135plus/"
cp "$REPO"/firmware/*.hex "$REPO/firmware/README.md" "$RES/firmware/" 2>/dev/null || true
cp -R "$REPO/profiles" "$REPO/oem" "$RES/"
cp "$REPO/LICENSE" "$REPO/NOTICE" "$RES/"
cp "$CACHE/AppIcon.icns" "$RES/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Pakon</string>
    <key>CFBundleDisplayName</key><string>Pakon</string>
    <key>CFBundleIdentifier</key><string>com.pakon.scanner</string>
    <key>CFBundleExecutable</key><string>Pakon</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>1.1.0</string>
    <key>LSMinimumSystemVersion</key><string>$DEPLOY</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.photography</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>AGPL-3.0-or-later. Personal build of OpenPakon 1.1.0. Not affiliated with Kodak or Pakon.</string>
</dict>
</plist>
PLIST
plutil -lint -s "$APP/Contents/Info.plist"

step "check"
file "$APP/Contents/MacOS/Pakon" | grep -q "x86_64" || { echo "binary is not x86_64" >&2; exit 1; }
for m in $(otool -l "$APP/Contents/MacOS/Pakon" | awk '/LC_BUILD_VERSION/{b=1} b&&/minos/{print $2; b=0}'); do
    [ "$(printf '%s\n%s\n' "$m" "$DEPLOY" | sort -V | tail -1)" = "$DEPLOY" ] \
        || { echo "needs macOS $m, newer than $DEPLOY" >&2; exit 1; }
done
if otool -L "$APP/Contents/MacOS/Pakon" | grep -E '^\s' | grep -v -E '^\s+(/System/Library/|/usr/lib/)'; then
    echo "links a library outside the system" >&2
    exit 1
fi

step "sign (ad hoc)"
# Hardened runtime needs the JIT entitlements or Unicorn cannot run.
codesign --force --options runtime --sign - --entitlements "$HERE/entitlements.plist" "$APP"
codesign --verify --deep --strict "$APP"

step "dmg"
STAGE="$CACHE/dmg-intel"
rm -rf "$STAGE" && mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/Pakon.app"
ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname Pakon -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$OUT/Pakon.dmg"
rm -rf "$STAGE"

step "done"
du -sh "$APP" "$OUT/Pakon.dmg"
echo "App: $APP"
