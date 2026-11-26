#!/usr/bin/env bash
# Build target/Cardumen.app; with --install, replace /Applications/Cardumen.app.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
app="target/Cardumen.app"

cargo build --release

rm -rf "$app" target/Cardumen.iconset
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/cardumen "$app/Contents/MacOS/cardumen"

python3 scripts/icon.py "$root" target/Cardumen.iconset
iconutil -c icns target/Cardumen.iconset -o "$app/Contents/Resources/Cardumen.icns"
rm -rf target/Cardumen.iconset

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Cardumen</string>
  <key>CFBundleDisplayName</key><string>Cardumen</string>
  <key>CFBundleIdentifier</key><string>dev.eduardoalvarez.cardumen</string>
  <key>CFBundleExecutable</key><string>cardumen</string>
  <key>CFBundleIconFile</key><string>Cardumen</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --sign - "$app"
echo "built $app ($version)"

if [[ "${1:-}" == "--install" ]]; then
  rm -rf /Applications/Cardumen.app
  cp -R "$app" /Applications/Cardumen.app
  echo "installed /Applications/Cardumen.app"
fi
