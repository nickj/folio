#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build.sh
mkdir -p "$HOME/bin/apps" "$HOME/bin"
if [[ -e "$HOME/bin/md" ]] && ! cmp -s scripts/md "$HOME/bin/md"; then
  printf 'Refusing to overwrite an unrelated ~/bin/md. Back it up first.\n' >&2
  exit 1
fi
if pgrep -x folio >/dev/null; then
  printf 'Quit Folio before installing a new build.\n' >&2
  exit 1
fi
# Stage the whole bundle on the destination volume before replacing the previous install.
stage=$(mktemp -d "$HOME/bin/apps/.folio-install.XXXXXX")
trap 'rm -rf "$stage"' EXIT
ditto dist/Folio.app "$stage/Folio.app"
if [[ -d "$HOME/bin/apps/Folio.app" ]]; then mv "$HOME/bin/apps/Folio.app" "$stage/Previous.app"; fi
mv "$stage/Folio.app" "$HOME/bin/apps/Folio.app"
install -m 755 scripts/md "$HOME/bin/md"
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
# Avoid two bundles with the same id competing in Finder’s Open With list.
"$lsregister" -u "$PWD/dist/Folio.app" || true
"$lsregister" -f "$HOME/bin/apps/Folio.app"
if [[ "${1:-}" == "--default" ]]; then swift scripts/default-handler.swift; fi
printf '\nInstalled ~/bin/apps/Folio.app and ~/bin/md\n'
