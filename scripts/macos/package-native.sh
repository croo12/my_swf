#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
version="${1:-0.2.0}"
app="target/native-bundle/Planning Workbench.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create target/aarch64-apple-darwin/release/factory-workbench \
  target/x86_64-apple-darwin/release/factory-workbench -output "$app/Contents/MacOS/factory-workbench"
cp apps/client/src-tauri/icons/icon.icns "$app/Contents/Resources/icon.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Planning Workbench</string>
<key>CFBundleIdentifier</key><string>dev.softwarefactory.native</string>
<key>CFBundleExecutable</key><string>factory-workbench</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${version#v}</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PLIST
# Ad-hoc signature only; distributed releases still need Developer ID signing/notarization.
codesign --force --deep --sign - "$app"
ditto -c -k --sequesterRsrc --keepParent "$app" "target/Planning-Workbench-${version}-macos-universal.zip"
