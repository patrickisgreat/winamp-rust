#!/bin/bash
# Build Winamp.app (macOS bundle) and install it.
# Usage: scripts/make-app.sh [install-dir]   (default: /Applications, falls back to ~/Applications)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_DIR="${1:-/Applications}"
cd "$ROOT"
cargo build --release
cargo run -q --release -p winamp-skin --example icon -- target/icon.png 1024 >/dev/null

APP="$ROOT/target/Winamp.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/winamp "$APP/Contents/MacOS/winamp"

# Icon: build an .icns from the rendered PNG.
ICONSET="$ROOT/target/Winamp.iconset"
rm -rf "$ICONSET"; mkdir -p "$ICONSET"
for sz in 16 32 64 128 256 512; do
  sips -z $sz $sz target/icon.png --out "$ICONSET/icon_${sz}x${sz}.png" >/dev/null
  dbl=$((sz*2))
  sips -z $dbl $dbl target/icon.png --out "$ICONSET/icon_${sz}x${sz}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Winamp.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Winamp</string>
  <key>CFBundleDisplayName</key><string>Winamp</string>
  <key>CFBundleIdentifier</key><string>org.winamp-rust.Winamp</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>winamp</string>
  <key>CFBundleIconFile</key><string>Winamp</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Audio file</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSItemContentTypes</key>
      <array><string>public.audio</string><string>public.mp3</string><string>public.mpeg-4-audio</string><string>org.xiph.flac</string><string>org.xiph.ogg-audio</string><string>com.microsoft.waveform-audio</string><string>public.aiff-audio</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Playlist</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleTypeExtensions</key><array><string>m3u</string><string>m3u8</string><string>pls</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Winamp skin</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleTypeExtensions</key><array><string>wsz</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

# Ad-hoc sign so Gatekeeper is happy with a locally built app.
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true

if [ -w "$DEST_DIR" ]; then :; else DEST_DIR="$HOME/Applications"; mkdir -p "$DEST_DIR"; fi
rm -rf "$DEST_DIR/Winamp.app"
cp -R "$APP" "$DEST_DIR/Winamp.app"
echo "installed $DEST_DIR/Winamp.app"
