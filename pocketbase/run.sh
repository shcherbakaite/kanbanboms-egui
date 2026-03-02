#!/bin/bash
# Download and run PocketBase for KanbanBOMs.
# Run from project root: ./pocketbase/run.sh

set -e
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

PB="$DIR/pocketbase"
VERSION="0.22.0"
ZIP="pocketbase_${VERSION}_linux_amd64.zip"
URL="https://github.com/pocketbase/pocketbase/releases/download/v${VERSION}/${ZIP}"

if [ ! -f "$PB" ]; then
  echo "Downloading PocketBase v${VERSION}..."
  curl -sL -o "$ZIP" "$URL"
  unzip -o "$ZIP"
  rm -f "$ZIP"
  chmod +x "$PB"
  echo "Downloaded."
fi

echo "Starting PocketBase..."
exec "$PB" serve --http=0.0.0.0:8090
