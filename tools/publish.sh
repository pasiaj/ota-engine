#!/usr/bin/env bash
# Copy the static viewer into an xpost checkout: <xpost>/nginx/www/pasiaj.com/ota/sim/
# usage: tools/publish.sh /path/to/xpost
# It only copies files; commit and push in the xpost checkout yourself.
set -euo pipefail
src="$(cd "$(dirname "$0")/../viewer" && pwd)"
xpost="${1:?usage: $0 /path/to/xpost-checkout}"
dest="$xpost/nginx/www/pasiaj.com/ota/sim"
[ -d "$xpost/nginx/www/pasiaj.com" ] || { echo "not an xpost checkout: $xpost" >&2; exit 1; }
mkdir -p "$dest"
rsync -a --delete --exclude '_*' --exclude '.DS_Store' "$src/" "$dest/"
echo "viewer copied to $dest ($(find "$dest" -type f | wc -l | tr -d ' ') files)"
