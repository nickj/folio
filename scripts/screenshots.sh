#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --locked
mkdir -p target/readme docs/images target/Folio.iconset
swift scripts/icon.swift target/Folio.iconset
cp target/Folio.iconset/icon_256x256@2x.png docs/images/folio.png

render() {
  local name="$1" scene="$2" theme="$3" size="$4"
  target/release/folio --scene "$scene" --theme "$theme" --size "$size" --scale 2 \
    --snap "target/readme/$name-raw.png"
  target/release/folio --scene "$scene" --theme "$theme" --size "$size" --scale 2 \
    --chrome --snap "target/readme/$name-chrome.png"
  swift scripts/screenshot-frame.swift \
    "target/readme/$name-raw.png" "target/readme/$name-chrome.png" "docs/images/$name.png" "$theme"
}
render preview showcase light 1100x820
render split showcase-split dark 1200x820
printf '\nREADME images written to docs/images/\n'
