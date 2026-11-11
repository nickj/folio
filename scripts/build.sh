#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --locked
app="$PWD/dist/Folio.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" target/Folio.iconset
swift scripts/icon.swift target/Folio.iconset
iconutil -c icns target/Folio.iconset -o "$app/Contents/Resources/Folio.icns"
cp target/release/folio "$app/Contents/MacOS/folio"
cp macos/Info.plist "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
codesign --force --sign - "$app"
printf '\nBuilt %s\n' "$app"
