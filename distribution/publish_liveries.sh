#!/bin/sh
# distribution/publish_liveries.sh [liveries folder]
# The liveries live in one release of their own, tagged "liveries", never marked Latest; the
# installer's Liveries page reads the newest release that carries liveries.json. Rerun it after
# adding or changing a livery: it rebuilds every zip, uploads them and drops files it no longer has.
set -e
REPO=Vihaan2012-cmyk/A380-For-X-Plane-12-Based-on-The-FBW-A380
TAG=liveries
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${1:-/d/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/liveries}"
DIST="$ROOT/dist/liveries"

if [ -f "$DIST/liveries.json" ] && ls "$DIST"/livery-*.zip >/dev/null 2>&1; then
  echo "== using the existing liveries in $DIST (delete it to rebuild)"
else
  python "$ROOT/tools/livery_release.py" "$SRC" "$DIST"
fi

gh repo view "$REPO" >/dev/null 2>&1 || { echo "$REPO is not reachable with gh; log in with gh auth login" >&2; exit 1; }
if ! gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  gh release create "$TAG" --repo "$REPO" --latest=false --title "Liveries" \
    --notes "The liveries for the installer's Liveries page, each in 2K and 4K. Install them from the installer; every livery belongs to its author and is shared with their permission."
fi
ONLINE="$(gh release view "$TAG" --repo "$REPO" --json assets -q '.assets[] | "\(.name) \(.digest)"')"
set -- "$DIST"/livery-*.zip "$DIST"/livery-*.jpg
TOTAL=$#
N=0
for FILE in "$@"; do
  N=$((N + 1))
  NAME="$(basename "$FILE")"
  if echo "$ONLINE" | grep -qx "$NAME sha256:$(sha256sum "$FILE" | cut -d' ' -f1)"; then
    echo "[$N/$TOTAL] $NAME unchanged"
    continue
  fi
  echo "[$N/$TOTAL] $NAME ($(( $(wc -c < "$FILE") / 1048576 )) MB)"
  gh release upload "$TAG" "$FILE" --repo "$REPO" --clobber
done
gh release upload "$TAG" "$DIST/liveries.json" --repo "$REPO" --clobber
for NAME in $(gh release view "$TAG" --repo "$REPO" --json assets -q '.assets[].name'); do
  [ -f "$DIST/$NAME" ] || { echo "removing $NAME"; gh release delete-asset "$TAG" "$NAME" --repo "$REPO" -y; }
done
echo
echo "Published $TOTAL livery files to https://github.com/$REPO/releases/tag/$TAG"
